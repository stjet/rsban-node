use rsnano_core::{Amount, BlockEnum};

use std::{
    cmp::{max, Ordering},
    collections::{BTreeSet, VecDeque},
    sync::{Arc, RwLock},
};

/// Information on the value type
#[derive(Clone, Debug)]
pub struct ValueType {
    pub time: u64,
    pub block: Arc<RwLock<BlockEnum>>,
}

impl Ord for ValueType {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut result = self.time.cmp(&other.time);
        if result == Ordering::Equal {
            let block1 = self.block.read().unwrap();
            let block2 = other.block.read().unwrap();
            result = block1.hash().number().cmp(&block2.hash().number())
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
    pub fn new(time: u64, block: Arc<RwLock<BlockEnum>>) -> Self {
        Self { time, block }
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
#[derive(Clone)]
pub struct Buckets {
    /// container for the buckets to be read in round robin fashion
    buckets: VecDeque<BTreeSet<ValueType>>,

    /// thresholds that define the bands for each bucket, the minimum balance an account must have to enter a bucket,
    /// the container writes a block to the lowest indexed bucket that has balance larger than the bucket's minimum value
    minimums: VecDeque<u128>,

    /// Contains bucket indicies to iterate over when making the next scheduling decision
    schedule: VecDeque<u8>,

    /// index of bucket to read next
    current: usize,

    /// maximum number of blocks in whole container, each bucket's maximum is maximum / bucket_number
    maximum: u64,
}

impl Buckets {
    /// Prioritization constructor, construct a container containing approximately 'maximum' number of blocks.
    pub fn new(maximum: u64) -> Self {
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

        let buckets = VecDeque::from(vec![BTreeSet::new(); minimums.len()]);

        let mut schedule = VecDeque::with_capacity(buckets.len());
        for i in 0..buckets.len() {
            schedule.push_back(i as u8);
        }

        Self {
            buckets,
            minimums,
            schedule,
            current: 0,
            maximum,
        }
    }

    /// Returns the total number of blocks in buckets
    pub fn size(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    /// Moves the bucket pointer to the next bucket
    fn next(&mut self) {
        self.current += 1;
        if self.current >= self.schedule.len() {
            self.current = 0;
        }
    }

    /// Pop the current block from the container and seek to the next block, if it exists
    pub fn pop(&mut self) {
        debug_assert!(!self.empty());
        debug_assert!(!self.buckets[self.current].is_empty());
        let bucket = &mut self.buckets[self.current];
        if let Some(first) = bucket.iter().next().cloned() {
            bucket.remove(&first);
        }
        self.seek();
    }

    /// Seek to the next non-empty bucket, if one exists
    pub fn seek(&mut self) {
        self.next();
        for _ in 0..self.schedule.len() {
            if self.buckets[self.current].is_empty() {
                self.next();
            }
        }
    }

    /// Return the highest priority block of the current bucket
    pub fn top(&mut self) -> &Arc<RwLock<BlockEnum>> {
        debug_assert!(!self.empty());
        debug_assert!(!self.buckets[self.current].is_empty());

        &self.buckets[self.current].iter().next().unwrap().block
    }

    /// Push a block and its associated time into the prioritization container.
    /// The time is given here because sideband might not exist in the case of state blocks.
    pub fn push(&mut self, time: u64, block: Arc<RwLock<BlockEnum>>, priority: Amount) {
        let was_empty = self.empty();
        let index = self.index(&priority);
        let bucket_count = self.buckets.len();
        let bucket = &mut self.buckets[index];
        bucket.insert(ValueType::new(time, block));

        if bucket.len() > max(1, self.maximum as usize / bucket_count) {
            let end = bucket.iter().next_back().cloned().unwrap();
            bucket.remove(&end);
        }

        if was_empty {
            self.seek();
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
    pub fn empty(&self) -> bool {
        self.buckets.iter().all(|b| b.is_empty())
    }

    pub fn dump(&self) {
        for i in &self.buckets {
            for j in i.iter() {
                eprintln!("{} {}", j.time, j.block.read().unwrap().hash());
            }
        }
        eprintln!("current: {}", self.schedule[self.current]);
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
}
