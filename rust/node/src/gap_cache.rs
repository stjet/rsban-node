use crate::{
    config::{NodeConfig, NodeFlags},
    consensus::Vote,
    OnlineReps,
};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    Account, Amount, BlockHash,
};
use rsnano_ledger::Ledger;
use std::{
    collections::{BTreeMap, HashMap},
    mem::size_of,
    sync::{Arc, Mutex},
    time::SystemTime,
};

const MAX: usize = 256;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
struct GapInformation {
    arrival: SystemTime,
    hash: BlockHash,
    voters: Vec<Account>, // todo: Should this be a HashSet?
    bootstrap_started: bool,
}

impl GapInformation {
    fn new(arrival: SystemTime, hash: BlockHash) -> Self {
        Self {
            arrival,
            hash,
            voters: Vec::new(),
            bootstrap_started: false,
        }
    }

    fn size() -> usize {
        size_of::<i64>() + size_of::<BlockHash>() + size_of::<Account>() + size_of::<bool>()
    }

    #[cfg(test)]
    fn create_test_instance() -> Self {
        Self {
            arrival: SystemTime::UNIX_EPOCH,
            hash: BlockHash::from(42),
            voters: Vec::new(),
            bootstrap_started: false,
        }
    }
}

struct OrderedGaps {
    gap_infos: HashMap<BlockHash, GapInformation>,
    by_arrival: BTreeMap<SystemTime, Vec<BlockHash>>,
}

impl OrderedGaps {
    fn new() -> Self {
        Self {
            gap_infos: HashMap::new(),
            by_arrival: BTreeMap::new(),
        }
    }

    fn len(&self) -> usize {
        self.gap_infos.len()
    }

    fn add(&mut self, gap_info: GapInformation) {
        let hash = gap_info.hash;
        let arrival = gap_info.arrival;
        if let Some(previous) = self.gap_infos.insert(gap_info.hash, gap_info) {
            self.remove_arrival(previous.arrival, &previous.hash);
        }

        self.add_arrival(arrival, hash);
    }

    fn get(&self, hash: &BlockHash) -> Option<&GapInformation> {
        self.gap_infos.get(hash)
    }

    fn modify(
        &mut self,
        hash: &BlockHash,
        modify_callback: &mut dyn FnMut(&mut GapInformation),
    ) -> bool {
        if let Some(info) = self.gap_infos.get_mut(hash) {
            let old_arrival = info.arrival;
            modify_callback(info);
            if info.arrival != old_arrival {
                let hash = info.hash;
                let new_arrival = info.arrival;
                self.remove_arrival(old_arrival, &hash);
                self.add_arrival(new_arrival, hash);
            }
            true
        } else {
            false
        }
    }

    fn remove(&mut self, hash: &BlockHash) -> Option<GapInformation> {
        if let Some(gap_info) = self.gap_infos.remove(hash) {
            self.remove_arrival(gap_info.arrival, hash);
            Some(gap_info)
        } else {
            None
        }
    }

    fn add_arrival(&mut self, arrival: SystemTime, hash: BlockHash) {
        let hashes = self.by_arrival.entry(arrival).or_default();
        hashes.push(hash);
    }

    fn remove_arrival(&mut self, arrival: SystemTime, hash: &BlockHash) {
        let hashes = self.by_arrival.get_mut(&arrival).unwrap();
        if hashes.len() == 1 {
            self.by_arrival.remove(&arrival);
        } else {
            hashes.retain(|h| h != hash)
        }
    }

    fn trim(&mut self, max: usize) {
        while self.len() > max {
            let (_, hashes) = self.by_arrival.pop_first().unwrap();
            for hash in hashes {
                self.gap_infos.remove(&hash);
            }
        }
    }

    fn earliest(&self) -> Option<SystemTime> {
        self.by_arrival
            .first_key_value()
            .map(|(&arrival, _)| arrival)
    }

    pub fn size_of_element() -> usize {
        size_of::<BlockHash>() * 2 + GapInformation::size() + size_of::<i64>()
    }
}

pub struct GapCache {
    node_config: Arc<NodeConfig>,
    online_reps: Arc<Mutex<OnlineReps>>,
    ledger: Arc<Ledger>,
    node_flags: Arc<NodeFlags>,
    blocks: Mutex<OrderedGaps>,
    start_bootstrap_callback: Box<dyn Fn(BlockHash)>,
}

impl GapCache {
    pub fn new(
        node_config: Arc<NodeConfig>,
        online_reps: Arc<Mutex<OnlineReps>>,
        ledger: Arc<Ledger>,
        node_flags: Arc<NodeFlags>,
        start_bootstrap_callback: Box<dyn Fn(BlockHash)>,
    ) -> Self {
        Self {
            node_config,
            online_reps,
            ledger,
            node_flags,
            blocks: Mutex::new(OrderedGaps::new()),
            start_bootstrap_callback,
        }
    }

    pub fn add(&mut self, hash: &BlockHash, time_point: SystemTime) {
        let mut lock = self.blocks.lock().unwrap();
        let modified = lock.modify(hash, &mut |info| {
            info.arrival = time_point;
        });

        if !modified {
            let gap_information = GapInformation::new(time_point, *hash);
            lock.add(gap_information);
            lock.trim(MAX);
        }
    }

    pub fn erase(&mut self, hash: &BlockHash) {
        let mut lock = self.blocks.lock().unwrap();
        lock.remove(hash);
    }

    pub fn vote(&mut self, vote: &Vote) {
        let mut lock = self.blocks.lock().unwrap();
        for hash in &vote.hashes {
            lock.modify(hash, &mut |gap_info| {
                if !gap_info.bootstrap_started {
                    let is_new = !gap_info.voters.iter().any(|v| *v == vote.voting_account);
                    if is_new {
                        gap_info.voters.push(vote.voting_account);

                        if self.bootstrap_check(&gap_info.voters, hash) {
                            gap_info.bootstrap_started = true;
                        }
                    }
                }
            });
        }
    }

    pub fn bootstrap_check(&self, voters: &[Account], hash: &BlockHash) -> bool {
        let tally = Amount::raw(
            voters
                .iter()
                .map(|voter| self.ledger.weight(voter).number())
                .sum(),
        );

        let start_bootstrap = if !self.node_flags.disable_lazy_bootstrap {
            tally >= self.online_reps.lock().unwrap().delta()
        } else if !self.node_flags.disable_legacy_bootstrap {
            tally > self.bootstrap_threshold()
        } else {
            false
        };

        if start_bootstrap && !self.ledger.block_or_pruned_exists(hash) {
            self.bootstrap_start(*hash);
        }

        start_bootstrap
    }

    pub fn bootstrap_start(&self, hash_a: BlockHash) {
        (self.start_bootstrap_callback)(hash_a);
    }

    pub fn bootstrap_threshold(&self) -> Amount {
        Amount::raw(
            (self.online_reps.lock().unwrap().trended().number() / 256)
                * self.node_config.bootstrap_fraction_numerator as u128,
        )
    }

    pub fn size(&self) -> usize {
        self.blocks.lock().unwrap().len()
    }

    pub fn block_exists(&self, hash: &BlockHash) -> bool {
        let lock = self.blocks.lock().unwrap();
        lock.get(hash).is_some()
    }

    pub fn earliest(&self) -> Option<SystemTime> {
        self.blocks.lock().unwrap().earliest()
    }

    pub fn block_arrival(&self, hash: &BlockHash) -> SystemTime {
        self.blocks.lock().unwrap().get(hash).unwrap().arrival
    }

    pub fn collect_container_info(&self, name: String) -> ContainerInfoComponent {
        let children = vec![ContainerInfoComponent::Leaf(ContainerInfo {
            name: "blocks".to_owned(),
            count: self.size(),
            sizeof_element: OrderedGaps::size_of_element(),
        })];

        ContainerInfoComponent::Composite(name, children)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn new_ordered_gaps_is_empty() {
        let gaps = OrderedGaps::new();
        assert_eq!(gaps.len(), 0);
        assert_eq!(gaps.earliest(), None);
    }

    #[test]
    fn add_gap_information_to_ordered_gaps() {
        let mut gaps = OrderedGaps::new();
        gaps.add(GapInformation::create_test_instance());
        assert_eq!(gaps.len(), 1);
    }

    #[test]
    fn remove_existing_gap_information_of_ordered_gaps() {
        let mut gaps = OrderedGaps::new();
        let gap_info = GapInformation::create_test_instance();
        let hash = gap_info.hash;
        gaps.add(gap_info);
        assert_eq!(gaps.len(), 1);
        assert!(gaps.remove(&hash).is_some());
        assert_eq!(gaps.len(), 0);
    }

    #[test]
    fn try_to_remove_non_existing_gap_information_of_ordered_gaps() {
        let mut gaps = OrderedGaps::new();
        let gap_info = GapInformation::create_test_instance();
        let hash = BlockHash::from(10);
        assert!(hash != gap_info.hash);
        gaps.add(gap_info);
        assert_eq!(gaps.len(), 1);
        assert!(gaps.remove(&hash).is_none());
        assert_eq!(gaps.len(), 1);
    }

    #[test]
    fn get_gap_info_by_block_hash() {
        let mut gaps = OrderedGaps::new();
        let gap_info = GapInformation::create_test_instance();

        gaps.add(gap_info.clone());

        let result = gaps.get(&gap_info.hash).unwrap();
        assert_eq!(result, &gap_info);
    }

    #[test]
    fn add_same_gap_information_to_ordered_gaps_twice_replaces_the_first_insert() {
        let mut gaps = OrderedGaps::new();
        let gap_info = GapInformation::create_test_instance();
        gaps.add(gap_info.clone());
        gaps.add(gap_info);
        assert_eq!(gaps.len(), 1);
    }

    #[test]
    fn trim_removes_oldest_entries() {
        let mut gaps = OrderedGaps::new();

        // will be removed by trim
        gaps.add(GapInformation {
            hash: BlockHash::from(1),
            arrival: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            ..GapInformation::create_test_instance()
        });

        // will be kept
        gaps.add(GapInformation {
            hash: BlockHash::from(3),
            arrival: SystemTime::UNIX_EPOCH + Duration::from_secs(2),
            ..GapInformation::create_test_instance()
        });

        // will be kept
        gaps.add(GapInformation {
            hash: BlockHash::from(4),
            arrival: SystemTime::UNIX_EPOCH + Duration::from_secs(3),
            ..GapInformation::create_test_instance()
        });

        // will be removed by trim
        gaps.add(GapInformation {
            hash: BlockHash::from(2),
            arrival: SystemTime::UNIX_EPOCH,
            ..GapInformation::create_test_instance()
        });

        gaps.trim(2);

        assert_eq!(gaps.len(), 2);
        assert!(gaps.get(&BlockHash::from(3)).is_some());
        assert!(gaps.get(&BlockHash::from(4)).is_some());
        assert_eq!(
            gaps.earliest(),
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(2))
        );
    }

    #[test]
    fn can_modify_gap_information() {
        let mut gaps = OrderedGaps::new();
        let hash = BlockHash::from(4);
        gaps.add(GapInformation {
            hash,
            bootstrap_started: false,
            ..GapInformation::create_test_instance()
        });

        gaps.modify(&hash, &mut |info| info.bootstrap_started = true);

        assert!(gaps.get(&hash).unwrap().bootstrap_started);
    }

    #[test]
    fn gap_information_can_have_same_arrival_time() {
        let mut gaps = OrderedGaps::new();
        gaps.add(GapInformation {
            hash: BlockHash::from(1),
            arrival: SystemTime::UNIX_EPOCH,
            ..GapInformation::create_test_instance()
        });

        gaps.add(GapInformation {
            hash: BlockHash::from(2),
            arrival: SystemTime::UNIX_EPOCH,
            ..GapInformation::create_test_instance()
        });

        gaps.remove(&BlockHash::from(1));

        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps.earliest(), Some(SystemTime::UNIX_EPOCH));
    }

    #[test]
    fn can_modify_arrival() {
        let mut gaps = OrderedGaps::new();
        gaps.add(GapInformation {
            hash: BlockHash::from(1),
            arrival: SystemTime::UNIX_EPOCH,
            ..GapInformation::create_test_instance()
        });

        gaps.modify(&BlockHash::from(1), &mut |info| {
            info.arrival = SystemTime::UNIX_EPOCH + Duration::from_secs(10)
        });

        assert_eq!(gaps.len(), 1);
        assert_eq!(
            gaps.earliest(),
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(10))
        );
    }
}
