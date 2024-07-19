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
    minimum_balance: Amount,
    maximum: usize,
    queue: BTreeSet<ValueType>,
}

impl Bucket {
    pub fn new(maximum: usize, minimum_balance: Amount) -> Self {
        Self {
            maximum,
            minimum_balance,
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

    /// Returns true if the block was inserted
    pub fn push(&mut self, time: u64, block: Arc<BlockEnum>) -> bool {
        let added = self.queue.insert(ValueType::new(time, block));
        if self.queue.len() > self.maximum {
            self.queue.pop_last();
        }
        added
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

    /// index of bucket to read next
    current: usize,
}

impl Buckets {
    /// Prioritization constructor, construct a container containing approximately 'maximum' number of blocks.
    /// @param maximum number of blocks that this container can hold, this is a soft and approximate limit.
    pub fn new(maximum: usize) -> Self {
        Self {
            buckets: Self::create_buckets(maximum),
            current: 0,
        }
    }

    fn create_buckets(maximum: usize) -> VecDeque<Bucket> {
        let mut buckets = VecDeque::new();
        const SIZE_EXPECTED: usize = 63;
        let bucket_max = max(1, maximum / SIZE_EXPECTED);
        let mut build_region = |begin: u128, end: u128, count: usize| {
            let width = (end - begin) / (count as u128);
            for i in 0..count {
                let minimum_balance = begin + (i as u128 * width);
                buckets.push_back(Bucket::new(bucket_max, minimum_balance.into()))
            }
        };

        build_region(0, 1 << 79, 1);
        build_region(1 << 79, 1 << 88, 1);
        build_region(1 << 88, 1 << 92, 2);
        build_region(1 << 92, 1 << 96, 4);
        build_region(1 << 96, 1 << 100, 8);
        build_region(1 << 100, 1 << 104, 16);
        build_region(1 << 104, 1 << 108, 16);
        build_region(1 << 108, 1 << 112, 8);
        build_region(1 << 112, 1 << 116, 4);
        build_region(1 << 116, 1 << 120, 2);
        build_region(1 << 120, 1 << 127, 1);

        buckets
    }

    /// Push a block and its associated time into the prioritization container.
    /// The time is given here because sideband might not exist in the case of state blocks.
    pub fn push(&mut self, time: u64, block: Arc<BlockEnum>, priority: Amount) -> bool {
        let was_empty = self.is_empty();
        let added = self.find_bucket(priority).push(time, block);

        if was_empty {
            self.seek();
        }
        added
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

    pub fn find_bucket(&mut self, amount: Amount) -> &mut Bucket {
        let mut bucket = None;

        for b in self.buckets.iter_mut() {
            if b.minimum_balance > amount {
                break;
            }
            bucket = Some(b);
        }

        // There should always be a bucket with a minimum_balance of 0
        bucket.unwrap()
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

impl Default for Buckets {
    fn default() -> Self {
        Self::new(250_000)
    }
}

#[cfg(test)]
mod tests {
    use rsnano_core::KeyPair;

    use super::*;

    #[test]
    fn construction() {
        let buckets = Buckets::default();
        assert_eq!(buckets.len(), 0);
        assert!(buckets.is_empty());
        assert_eq!(buckets.bucket_count(), 63);
    }

    #[test]
    fn insert_gxrb() {
        let mut buckets = Buckets::default();
        buckets.push(1000, test_block(1), Amount::nano(1000));
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets.bucket_size(49), 1);
    }

    #[test]
    fn insert_mxrb() {
        let mut buckets = Buckets::default();
        buckets.push(1000, test_block(1), Amount::nano(1));
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets.bucket_size(14), 1);
    }

    // Test two blocks with the same priority
    #[test]
    fn insert_same_priority() {
        let mut buckets = Buckets::default();
        buckets.push(1000, test_block(1), Amount::nano(1000));
        buckets.push(1000, test_block(2), Amount::nano(1000));
        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets.bucket_size(49), 2);
    }

    // Test the same block inserted multiple times
    #[test]
    fn insert_duplicate() {
        let mut buckets = Buckets::default();
        buckets.push(1000, test_block(1), Amount::nano(1000));
        buckets.push(1000, test_block(1), Amount::nano(1000));
        assert_eq!(buckets.len(), 1);
    }

    #[test]
    fn insert_older() {
        let mut buckets = Buckets::default();
        let a = test_block(1);
        let b = test_block(2);
        let a_hash = a.hash();
        let b_hash = b.hash();
        buckets.push(1000, a, Amount::nano(1000));
        buckets.push(1000, b, Amount::nano(1000));
        assert_eq!(buckets.top().hash(), a_hash);
        buckets.pop();
        assert_eq!(buckets.top().hash(), b_hash);
    }

    #[test]
    fn pop() {
        let mut buckets = Buckets::default();
        buckets.push(1000, test_block(1), Amount::nano(1000));
        buckets.pop();
        assert!(buckets.is_empty());
    }

    #[test]
    fn top_one() {
        let mut buckets = Buckets::default();
        let a = test_block(1);
        let a_hash = a.hash();
        buckets.push(1000, a, Amount::nano(1000));
        assert_eq!(buckets.top().hash(), a_hash);
    }

    #[test]
    fn top_two() {
        let mut buckets = Buckets::default();
        let a = test_block(1);
        let b = test_block(2);
        let a_hash = a.hash();
        let b_hash = b.hash();
        buckets.push(1000, a, Amount::nano(1000));
        buckets.push(1, b, Amount::nano(1));
        assert_eq!(buckets.top().hash(), a_hash);
        buckets.pop();
        assert_eq!(buckets.top().hash(), b_hash);
    }

    #[test]
    fn top_round_robin() {
        let mut buckets = Buckets::default();
        let a = test_block(1);
        let b = test_block(2);
        let c = test_block(2);
        let d = test_block(2);
        let a_hash = a.hash();
        let b_hash = b.hash();
        let c_hash = c.hash();
        let d_hash = d.hash();
        buckets.push(1000, a, Amount::zero());
        buckets.push(1000, b, Amount::nano(1000));
        buckets.push(1000, c, Amount::nano(1));
        buckets.push(1100, d, Amount::nano(1));
        assert_eq!(buckets.top().hash(), a_hash);
        buckets.pop();
        assert_eq!(buckets.top().hash(), c_hash);
        buckets.pop();
        assert_eq!(buckets.top().hash(), b_hash);
        buckets.pop();
        assert_eq!(buckets.top().hash(), d_hash);
    }

    #[test]
    fn trim_normal() {
        let mut buckets = Buckets::new(2);
        let a = test_block(1);
        let a_hash = a.hash();
        let b = test_block(2);
        buckets.push(1000, a, Amount::nano(1000));
        buckets.push(1100, b, Amount::nano(1000));
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets.top().hash(), a_hash);
    }

    #[test]
    fn trim_reverse() {
        let mut buckets = Buckets::new(2);
        let a = test_block(1);
        let b = test_block(2);
        let b_hash = b.hash();
        buckets.push(1100, a, Amount::nano(1000));
        buckets.push(1000, b, Amount::nano(1000));
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets.top().hash(), b_hash);
    }

    #[test]
    fn trim_even() {
        let mut buckets = Buckets::new(2);
        let a = test_block(1);
        let b = test_block(2);
        let c = test_block(3);
        let a_hash = a.hash();
        let c_hash = c.hash();
        buckets.push(1000, a, Amount::nano(1000));
        buckets.push(1100, b, Amount::nano(1000));
        buckets.push(1000, c, Amount::nano(1));
        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets.top().hash(), a_hash);
        buckets.pop();
        assert_eq!(buckets.top().hash(), c_hash);
    }

    fn test_block(private_key: u64) -> Arc<BlockEnum> {
        Arc::new(BlockEnum::new_test_instance_with_key(KeyPair::from(
            private_key,
        )))
    }
}
