use super::{ActiveElections, Election, ElectionBehavior};
use crate::{consensus::ActiveElectionsExt, stats::Stats};
use rsnano_core::{Amount, BlockEnum, QualifiedRoot};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Arc, Mutex},
};

pub(crate) struct PriorityBucketConfig {
    /// Maximum number of blocks to sort by priority per bucket.
    max_blocks: usize,

    /// Number of guaranteed slots per bucket available for election activation.
    reserved_elections: usize,

    /// Maximum number of slots per bucket available for election activation if the active election count is below the configured limit. (node.active_elections.size)
    max_elections: usize,
}

impl Default for PriorityBucketConfig {
    fn default() -> Self {
        Self {
            max_blocks: 1024 * 8,
            reserved_elections: 100,
            max_elections: 150,
        }
    }
}

type Priority = u64;

/// A struct which holds an ordered set of blocks to be scheduled, ordered by their block arrival time
/// TODO: This combines both block ordering and election management, which makes the class harder to test. The functionality should be split.
pub(crate) struct NewBucket {
    minimum_balance: Amount,
    config: PriorityBucketConfig,
    active: Arc<ActiveElections>,
    stats: Arc<Stats>,
    data: Mutex<BucketData>,
}

impl NewBucket {
    fn available(&self) -> bool {
        let guard = self.data.lock().unwrap();
        if let Some(first) = guard.queue.first() {
            self.election_vacancy(first.time, &guard)
        } else {
            false
        }
    }

    fn election_vacancy(&self, candidate: Priority, data: &BucketData) -> bool {
        let election_count = data.elections.len();
        if election_count < self.config.reserved_elections {
            true
        } else if election_count < self.config.max_elections {
            self.active.vacancy(ElectionBehavior::Priority) > 0
        } else if election_count > 0 {
            let lowest = data.elections.lowest_priority();

            // Compare to equal to drain duplicates
            if candidate <= lowest {
                // Bound number of reprioritizations
                election_count < self.config.max_elections * 2
            } else {
                false
            }
        } else {
            false
        }
    }

    fn election_overfill(&self, data: &BucketData) -> bool {
        if data.elections.len() < self.config.reserved_elections {
            false
        } else if data.elections.len() < self.config.max_elections {
            self.active.vacancy(ElectionBehavior::Priority) < 0
        } else {
            true
        }
    }
}

trait BucketExt {
    fn activate(&self) -> bool;
}

impl BucketExt for Arc<NewBucket> {
    fn activate(&self) -> bool {
        let mut guard = self.data.lock().unwrap();

        let Some(top) = guard.queue.pop_first() else {
            return false; // Not activated;
        };

        let block = top.block;
        let priority = top.time;

        let self_w = Arc::downgrade(self);
        let erase_callback = Box::new(move |election: &Arc<Election>| {
            let Some(self_l) = self_w.upgrade() else {
                return;
            };
            let mut guard = self_l.data.lock().unwrap();
            guard.elections.erase(&election.qualified_root);
        });

        let result = self
            .active
            .insert(&block, ElectionBehavior::Priority, Some(erase_callback));

        todo!()
    }
}

struct BucketData {
    queue: BTreeSet<BlockEntry>,
    elections: OrderedElections,
}

struct BlockEntry {
    time: u64,
    block: Arc<BlockEnum>,
}

impl Ord for BlockEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        let time_order = self.time.cmp(&other.time);
        match time_order {
            Ordering::Equal => self.block.hash().cmp(&other.block.hash()),
            _ => time_order,
        }
    }
}

impl PartialOrd for BlockEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for BlockEntry {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && self.block.hash() == other.block.hash()
    }
}

impl Eq for BlockEntry {}

struct ElectionEntry {
    election: Arc<Election>,
    root: QualifiedRoot,
    priority: Priority,
}

#[derive(Default)]
struct OrderedElections {
    by_root: HashMap<QualifiedRoot, ElectionEntry>,
    sequenced: Vec<QualifiedRoot>,
    by_priority: BTreeMap<Priority, Vec<QualifiedRoot>>,
}

impl OrderedElections {
    fn insert(&mut self, entry: ElectionEntry) {
        let root = entry.root.clone();
        let priority = entry.priority;
        let old = self.by_root.insert(root.clone(), entry);
        assert!(old.is_none());
        self.sequenced.push(root.clone());
        self.by_priority.entry(priority).or_default().push(root);
    }

    fn lowest_priority(&self) -> u64 {
        self.by_priority
            .first_key_value()
            .map(|(prio, _)| *prio)
            .unwrap_or_default()
    }

    fn len(&self) -> usize {
        self.sequenced.len()
    }

    fn erase(&mut self, root: &QualifiedRoot) {
        if let Some(entry) = self.by_root.remove(root) {
            let keys = self.by_priority.get_mut(&entry.priority).unwrap();
            if keys.len() == 1 {
                self.by_priority.remove(&entry.priority);
            } else {
                keys.retain(|i| i != root);
            }
            self.sequenced.retain(|i| i != root);
        }
    }
}
