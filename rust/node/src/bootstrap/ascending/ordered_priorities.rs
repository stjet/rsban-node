use ordered_float::OrderedFloat;
use rand::{thread_rng, RngCore};
use rsnano_core::Account;
use std::{collections::BTreeMap, time::Instant};

pub(crate) struct PriorityEntry {
    pub account: Account,
    pub priority: OrderedFloat<f32>,
    pub timestamp: Option<Instant>,
    pub id: u64, // Uniformly distributed, used for random querying
}

impl PriorityEntry {
    pub fn new(account: Account, priority: OrderedFloat<f32>) -> Self {
        Self {
            account,
            priority,
            timestamp: None,
            id: thread_rng().next_u64(),
        }
    }
}

/// Tracks the ongoing account priorities
/// This only stores account priorities > 1.0f.
#[derive(Default)]
pub(crate) struct OrderedPriorities {
    by_id: BTreeMap<u64, PriorityEntry>,
    by_account: BTreeMap<Account, u64>,
    sequenced: Vec<u64>,
    by_priority: BTreeMap<OrderedFloat<f32>, Vec<u64>>,
}

impl OrderedPriorities {
    pub fn insert(&mut self, entry: PriorityEntry) -> bool {
        let id = entry.id;
        let account = entry.account;
        let priority = entry.priority;

        if self.by_id.contains_key(&entry.id) || self.by_account.contains_key(&account) {
            return false;
        }

        self.by_id.insert(id, entry);
        self.by_account.insert(account, id);
        self.sequenced.push(id);
        self.by_priority.entry(priority).or_default().push(id);
        true
    }

    pub fn change_priority(
        &mut self,
        account: &Account,
        mut f: impl FnMut(OrderedFloat<f32>) -> OrderedFloat<f32>,
    ) -> bool {
        if let Some(id) = self.by_account.get(account) {
            if let Some(entry) = self.by_id.get_mut(id) {
                let old_prio = entry.priority;
                entry.priority = f(entry.priority);
                let new_prio = entry.priority;
                if new_prio != old_prio {
                    let id = entry.id;
                    self.change_priority_internal(id, old_prio, new_prio)
                }
                return true;
            }
        }
        false
    }

    fn change_priority_internal(
        &mut self,
        id: u64,
        old_prio: OrderedFloat<f32>,
        new_prio: OrderedFloat<f32>,
    ) {
        if let Some(ids) = self.by_priority.get_mut(&old_prio) {
            if ids.len() == 1 {
                self.by_priority.remove(&old_prio);
            } else {
                ids.retain(|i| *i != id)
            }
        }
        self.by_priority.entry(new_prio).or_default().push(id);
    }
}
