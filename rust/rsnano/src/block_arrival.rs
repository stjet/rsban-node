use crate::BlockHash;
#[cfg(test)]
use mock_instant::Instant;
#[cfg(not(test))]
use std::time::Instant;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
    time::Duration,
};

pub(crate) struct BlockArrival {
    data: Mutex<BlockArrivalCache>,
    arrival_size_min: usize,
    arrival_time_min: Duration,
}

#[derive(Default)]
struct BlockArrivalCache {
    arrivals: BTreeMap<u64, BlockArrivalInfo>,
    by_hash: HashMap<BlockHash, u64>,
}

impl BlockArrivalCache {
    fn new_key(&self) -> u64 {
        self.arrivals
            .iter()
            .next_back()
            .map(|(&key, _)| key + 1)
            .unwrap_or_default()
    }

    fn add(&mut self, hash: &BlockHash, arrival: Instant) -> bool {
        if self.by_hash.contains_key(hash) {
            false
        } else {
            let key = self.new_key();
            self.arrivals.insert(
                key,
                BlockArrivalInfo {
                    arrival,
                    hash: *hash,
                },
            );
            self.by_hash.insert(*hash, key);
            true
        }
    }

    fn remove_old_entries(&mut self, min_size: usize, min_arrival_time: Duration) {
        let now = Instant::now();
        while self.arrivals.len() > min_size
            && self.first_entry().1.arrival + min_arrival_time < now
        {
            let (&key, _) = self.first_entry();
            if let Some(x) = self.arrivals.remove(&key) {
                self.by_hash.remove(&x.hash);
            }
        }
    }

    fn first_entry(&mut self) -> (&u64, &BlockArrivalInfo) {
        self.arrivals.iter().next().unwrap()
    }

    fn contains(&self, hash: &BlockHash) -> bool {
        self.by_hash.contains_key(hash)
    }
}

impl BlockArrival {
    pub(crate) fn new() -> Self {
        Self {
            data: Default::default(),
            arrival_size_min: 8 * 1024,
            arrival_time_min: Duration::from_secs(300),
        }
    }

    /// Return true to indicated an error if the block has already been inserted
    pub(crate) fn add(&self, hash: &BlockHash) -> bool {
        self.data.lock().unwrap().add(hash, Instant::now())
    }

    pub(crate) fn recent(&self, hash: &BlockHash) -> bool {
        let mut data_lk = self.data.lock().unwrap();
        data_lk.remove_old_entries(self.arrival_size_min, self.arrival_time_min);
        data_lk.contains(hash)
    }

    pub(crate) fn size(&self) -> usize {
        self.data.lock().unwrap().arrivals.len()
    }

    pub(crate) fn size_of_element(&self) -> usize {
        std::mem::size_of::<BlockArrivalInfo>()
    }
}

struct BlockArrivalInfo {
    arrival: Instant,
    hash: BlockHash,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mock_instant::MockClock;

    #[test]
    fn new_block_arrival() {
        let block_arrival = BlockArrival::new();
        assert_eq!(block_arrival.size(), 0);
        assert_eq!(block_arrival.arrival_size_min, 8192);
        assert_eq!(block_arrival.arrival_time_min, Duration::from_secs(300));
    }

    #[test]
    fn add_block() {
        let block_arrival = BlockArrival::new();
        block_arrival.add(&BlockHash::from(1));
        assert_eq!(block_arrival.size(), 1);
        block_arrival.add(&BlockHash::from(2));
        assert_eq!(block_arrival.size(), 2);
    }

    #[test]
    fn keep_recent_entries() {
        let mut block_arrival = BlockArrival::new();
        block_arrival.arrival_size_min = 2;
        block_arrival.arrival_time_min = Duration::from_secs(5);
        for i in 0..4 {
            block_arrival.add(&BlockHash::from(i as u64));
        }
        assert_eq!(block_arrival.size(), 4);
        block_arrival.recent(&BlockHash::from(0));
        assert_eq!(block_arrival.size(), 4);
    }

    #[test]
    fn remove_old_entries() {
        let mut block_arrival = BlockArrival::new();
        block_arrival.arrival_size_min = 2;
        block_arrival.arrival_time_min = Duration::from_secs(5);
        for i in 0..4 {
            block_arrival.add(&BlockHash::from(i as u64));
        }
        MockClock::advance(Duration::from_secs(6));
        assert_eq!(block_arrival.size(), 4);
        block_arrival.recent(&BlockHash::from(0));
        assert_eq!(block_arrival.size(), 2);
        assert_eq!(block_arrival.data.lock().unwrap().by_hash.len(), 2);
    }
}
