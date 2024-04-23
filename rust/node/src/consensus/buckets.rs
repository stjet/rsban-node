use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    Amount, BlockEnum,
};

use std::{
    cmp::{max, Ordering},
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};

/// Information on the value type
#[derive(Clone, Debug)]
pub struct ValueType {
    pub time: u64,
    pub block: Arc<BlockEnum>,
}

impl Ord for ValueType {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut result = self.time.cmp(&other.time);
        if result == Ordering::Equal {
            result = self.block.hash().number().cmp(&other.block.hash().number())
        }
        result
    }
}

impl PartialOrd for ValueType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ValueType {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ValueType {}

impl ValueType {
    pub fn new(time: u64, block: Arc<BlockEnum>) -> Self {
        Self { time, block }
    }
}

/// A class which holds an ordered set of blocks to be scheduled, ordered by their block arrival time
pub struct Bucket {
    maximum: usize,
    queue: BTreeSet<ValueType>,
}

impl Bucket {
    pub fn new(maximum: usize) -> Self {
        Self {
            maximum,
            queue: BTreeSet::new(),
        }
    }

    pub fn top(&self) -> &Arc<BlockEnum> {
        debug_assert!(!self.queue.is_empty());
        &self.queue.first().unwrap().block
    }

    pub fn pop(&mut self) {
        debug_assert!(!self.queue.is_empty());
        self.queue.pop_first();
    }

    pub fn push(&mut self, time: u64, block: Arc<BlockEnum>) {
        self.queue.insert(ValueType::new(time, block));
        if self.queue.len() > self.maximum {
            self.queue.pop_last();
        }
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn dump(&self) {
        for item in &self.queue {
            eprintln!("{} {}", item.time, item.block.hash());
        }
    }
}

/// A container for holding blocks and their arrival/creation time.
///
///  The container consists of a number of buckets. Each bucket holds an ordered set of 'ValueType' items.
///  The buckets are accessed in a round robin fashion. The index 'current' holds the index of the bucket to access next.
///  When a block is inserted, the bucket to go into is determined by the account balance and the priority inside that
///  bucket is determined by its creation/arrival time.
///
///  The arrival/creation time is only an approximation and it could even be wildly wrong,
///  for example, in the event of bootstrapped blocks.
///
pub struct Buckets {
    /// container for the buckets to be read in round robin fashion
    buckets: VecDeque<Bucket>,

    /// thresholds that define the bands for each bucket, the minimum balance an account must have to enter a bucket,
    /// the container writes a block to the lowest indexed bucket that has balance larger than the bucket's minimum value
    minimums: VecDeque<u128>,

    /// index of bucket to read next
    current: usize,

    /// maximum number of blocks in whole container, each bucket's maximum is maximum / bucket_number
    maximum: usize,
}

impl Buckets {
    /// Prioritization constructor, construct a container containing approximately 'maximum' number of blocks.
    pub fn new(maximum: usize) -> Self {
        let mut minimums = VecDeque::new();
        minimums.push_back(0);

        let mut build_region = |begin: u128, end: u128, count: usize| {
            let width = (end - begin) / (count as u128);
            for i in 0..count {
                minimums.push_back(begin + (i as u128 * width))
            }
        };

        build_region(1 << 88, 1 << 92, 2);
        build_region(1 << 92, 1 << 96, 4);
        build_region(1 << 96, 1 << 100, 8);
        build_region(1 << 100, 1 << 104, 16);
        build_region(1 << 104, 1 << 108, 16);
        build_region(1 << 108, 1 << 112, 8);
        build_region(1 << 112, 1 << 116, 4);
        build_region(1 << 116, 1 << 120, 2);
        minimums.push_back(1 << 120);

        let bucket_max = max(1, maximum / minimums.len());
        let buckets = minimums
            .iter()
            .map(|_| Bucket::new(bucket_max))
            .collect::<VecDeque<_>>();

        Self {
            buckets,
            minimums,
            current: 0,
            maximum,
        }
    }

    /// Push a block and its associated time into the prioritization container.
    /// The time is given here because sideband might not exist in the case of state blocks.
    pub fn push(&mut self, time: u64, block: Arc<BlockEnum>, priority: Amount) {
        let was_empty = self.is_empty();
        let index = self.index(&priority);
        let bucket = &mut self.buckets[index];
        bucket.push(time, block);

        if was_empty {
            self.seek();
        }
    }

    /// Moves the bucket pointer to the next bucket
    fn next(&mut self) {
        self.current += 1;
        if self.current >= self.buckets.len() {
            self.current = 0;
        }
    }

    /// Return the highest priority block of the current bucket
    pub fn top(&self) -> &Arc<BlockEnum> {
        debug_assert!(!self.is_empty());
        self.buckets[self.current].top()
    }

    /// Pop the current block from the container and seek to the next block, if it exists
    pub fn pop(&mut self) {
        debug_assert!(!self.is_empty());
        let bucket = &mut self.buckets[self.current];
        bucket.pop();
        self.seek();
    }

    /// Returns the total number of blocks in buckets
    pub fn len(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    /// Seek to the next non-empty bucket, if one exists
    pub fn seek(&mut self) {
        self.next();
        for _ in 0..self.buckets.len() {
            if self.buckets[self.current].is_empty() {
                self.next();
            } else {
                break;
            }
        }
    }

    /// Returns number of buckets, 129 by default
    pub fn bucket_count(&self) -> usize {
        self.buckets.len()
    }

    /// Returns number of items in bucket with index 'index'
    pub fn bucket_size(&self, index: usize) -> usize {
        self.buckets[index].len()
    }

    /// Returns true if all buckets are empty
    pub fn is_empty(&self) -> bool {
        self.buckets.iter().all(|b| b.is_empty())
    }

    pub fn dump(&self) {
        for bucket in &self.buckets {
            bucket.dump();
        }
        eprintln!("current: {}", self.current);
    }

    pub fn index(&self, amount: &Amount) -> usize {
        self.minimums
            .iter()
            .enumerate()
            .find_map(|(i, min)| {
                if amount.number() < *min {
                    Some(i)
                } else {
                    None
                }
            })
            .unwrap_or(self.minimums.len())
            - 1
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let leafs = self
            .buckets
            .iter()
            .enumerate()
            .map(|(i, b)| {
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: i.to_string(),
                    count: b.len(),
                    sizeof_element: 0,
                })
            })
            .collect();

        ContainerInfoComponent::Composite(name.into(), leafs)
    }
}

#[cfg(test)]
mod tests {
    use rsnano_core::KeyPair;

    use super::*;

    #[test]
    fn construction() {
        let buckets = Buckets::new(3);
        assert_eq!(buckets.len(), 0);
        assert!(buckets.is_empty());
        assert_eq!(buckets.bucket_count(), 62);
    }

    #[test]
    fn index_min() {
        let buckets = Buckets::new(3);
        assert_eq!(buckets.index(&Amount::zero()), 0);
    }

    #[test]
    fn index_max() {
        let buckets = Buckets::new(3);
        assert_eq!(buckets.index(&Amount::MAX), buckets.bucket_count() - 1);
    }

    #[test]
    fn insert_gxrb() {
        let mut buckets = Buckets::new(3);
        buckets.push(1000, block_a(), Amount::nano(1000));
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets.bucket_size(48), 1);
    }

    #[test]
    fn insert_mxrb() {
        let mut buckets = Buckets::new(3);
        buckets.push(1000, block_a(), Amount::nano(1));
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets.bucket_size(13), 1);
    }

    // Test two blocks with the same priority
    #[test]
    #[ignore]
    fn insert_same_priority() {
        let mut buckets = Buckets::new(3);
        buckets.push(1000, block_a(), Amount::nano(1000));
        buckets.push(1000, block_b(), Amount::nano(1000));
        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets.bucket_size(48), 2);
    }

    fn block_a() -> Arc<BlockEnum> {
        Arc::new(BlockEnum::create_test_instance_with_key(KeyPair::from(1)))
    }

    fn block_b() -> Arc<BlockEnum> {
        Arc::new(BlockEnum::create_test_instance_with_key(KeyPair::from(2)))
    }
}
