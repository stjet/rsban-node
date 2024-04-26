use ordered_float::OrderedFloat;
use rand::{thread_rng, RngCore};
use rsnano_core::Account;
use std::mem::size_of;
use std::ops::Bound;
use std::{collections::BTreeMap, time::Instant};

#[derive(Clone, Default)]
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
    pub const ELEMENT_SIZE: usize =
        size_of::<PriorityEntry>() + size_of::<Account>() + size_of::<f32>() + size_of::<u64>() * 4;

    pub fn len(&self) -> usize {
        self.sequenced.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sequenced.is_empty()
    }

    pub fn get(&self, account: &Account) -> Option<&PriorityEntry> {
        self.by_account
            .get(account)
            .and_then(|id| self.by_id.get(id))
    }

    pub fn contains(&self, account: &Account) -> bool {
        self.by_account.contains_key(account)
    }

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

    pub fn pop_lowest_priority(&mut self) -> Option<PriorityEntry> {
        if let Some(mut entry) = self.by_priority.first_entry() {
            let ids = entry.get_mut();
            let id = ids[0];
            if ids.len() == 1 {
                entry.remove();
            } else {
                ids.pop();
            }
            let entry = self.by_id.remove(&id).unwrap();
            self.sequenced.retain(|i| *i != entry.id);
            self.by_account.remove(&entry.account);
            Some(entry)
        } else {
            None
        }
    }

    pub fn change_timestamp(&mut self, account: &Account, timestamp: Option<Instant>) {
        if let Some(id) = self.by_account.get(account) {
            self.by_id.get_mut(id).unwrap().timestamp = timestamp;
        }
    }

    pub fn change_priority(
        &mut self,
        account: &Account,
        mut f: impl FnMut(OrderedFloat<f32>) -> Option<OrderedFloat<f32>>,
    ) -> bool {
        if let Some(&id) = self.by_account.get(account) {
            if let Some(entry) = self.by_id.get_mut(&id) {
                let old_prio = entry.priority;
                if let Some(new_prio) = f(entry.priority) {
                    entry.priority = new_prio;
                    if new_prio != old_prio {
                        let id = entry.id;
                        self.change_priority_internal(id, old_prio, new_prio)
                    }
                } else {
                    self.remove_id(id);
                }
                return true;
            }
        }
        false
    }

    pub fn wrapping_lower_bound(&self, value: u64) -> Option<&PriorityEntry> {
        let result = self
            .by_id
            .range((Bound::Included(value), Bound::Unbounded))
            .map(|(_, v)| v)
            .next();

        if result.is_none() {
            self.by_id.first_key_value().map(|(_, v)| v)
        } else {
            result
        }
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

    pub fn remove(&mut self, account: &Account) -> Option<PriorityEntry> {
        if let Some(id) = self.by_account.remove(account) {
            let entry = self.by_id.remove(&id).unwrap();
            self.sequenced.retain(|i| *i != id);
            self.remove_priority(id, entry.priority);
            Some(entry)
        } else {
            None
        }
    }

    fn remove_id(&mut self, id: u64) {
        let entry = self.by_id.remove(&id).unwrap();
        self.by_account.remove(&entry.account);
        self.sequenced.retain(|i| *i != id);
        self.remove_priority(id, entry.priority);
    }

    fn remove_priority(&mut self, id: u64, priority: OrderedFloat<f32>) {
        let ids = self.by_priority.get_mut(&priority).unwrap();
        if ids.len() > 1 {
            ids.retain(|i| *i != id);
        } else {
            self.by_priority.remove(&priority);
        }
    }
}
