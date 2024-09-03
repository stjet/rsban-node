use super::priority::{Priority, PriorityKeyDesc};
use rsnano_core::Account;
use std::collections::VecDeque;
use std::mem::size_of;
use std::{collections::BTreeMap, time::Instant};

#[derive(Clone, Default)]
pub(crate) struct PriorityEntry {
    pub account: Account,
    pub priority: Priority,
    pub timestamp: Option<Instant>,
    pub id: u64, // Uniformly distributed, used for random querying
}

impl PriorityEntry {
    pub fn new(id: u64, account: Account, priority: Priority) -> Self {
        Self {
            account,
            priority,
            timestamp: None,
            id,
        }
    }
}

/// Tracks the ongoing account priorities
/// This only stores account priorities > 1.0f.
#[derive(Default)]
pub(crate) struct OrderedPriorities {
    by_id: BTreeMap<u64, PriorityEntry>,
    by_account: BTreeMap<Account, u64>,
    sequenced: VecDeque<u64>,
    by_priority: BTreeMap<PriorityKeyDesc, Vec<u64>>, // descending
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
        self.sequenced.push_back(id);
        self.by_priority
            .entry(priority.into())
            .or_default()
            .push(id);
        true
    }

    pub fn pop_front(&mut self) -> Option<PriorityEntry> {
        let id = self.sequenced.pop_front()?;
        Some(self.remove_id(id))
    }

    pub fn change_timestamp(&mut self, account: &Account, timestamp: Option<Instant>) {
        if let Some(id) = self.by_account.get(account) {
            self.by_id.get_mut(id).unwrap().timestamp = timestamp;
        }
    }

    pub fn change_priority(
        &mut self,
        account: &Account,
        mut f: impl FnMut(Priority) -> Option<Priority>,
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

    pub fn next_priority(
        &self,
        cutoff: Instant,
        filter: impl Fn(&Account) -> bool,
    ) -> Option<Account> {
        self.by_priority
            .values()
            .flatten()
            .map(|id| self.by_id.get(id).unwrap())
            .find(|entry| {
                if let Some(ts) = entry.timestamp {
                    if ts > cutoff {
                        return false;
                    }
                }
                filter(&entry.account)
            })
            .map(|e| e.account)
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

    fn change_priority_internal(&mut self, id: u64, old_prio: Priority, new_prio: Priority) {
        if let Some(ids) = self.by_priority.get_mut(&old_prio.into()) {
            if ids.len() == 1 {
                self.by_priority.remove(&old_prio.into());
            } else {
                ids.retain(|i| *i != id)
            }
        }
        self.by_priority
            .entry(new_prio.into())
            .or_default()
            .push(id);
    }

    fn remove_id(&mut self, id: u64) -> PriorityEntry {
        let entry = self.by_id.remove(&id).unwrap();
        self.by_account.remove(&entry.account);
        self.sequenced.retain(|i| *i != id);
        self.remove_priority(id, entry.priority);
        entry
    }

    fn remove_priority(&mut self, id: u64, priority: Priority) {
        let ids = self.by_priority.get_mut(&priority.into()).unwrap();
        if ids.len() > 1 {
            ids.retain(|i| *i != id);
        } else {
            self.by_priority.remove(&priority.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let mut priorities = OrderedPriorities::default();
        assert_eq!(priorities.len(), 0);
        assert!(priorities.is_empty());
        assert!(priorities.get(&Account::from(1)).is_none());
        assert_eq!(priorities.contains(&Account::from(1)), false);
        assert!(priorities.pop_front().is_none());
        assert!(priorities.remove(&Account::from(1)).is_none());
    }

    #[test]
    fn insert_one() {
        let mut priorities = OrderedPriorities::default();
        let account = Account::from(1);
        priorities.insert(PriorityEntry::new(42, account, Priority::new(2.5)));
        assert_eq!(priorities.len(), 1);
        assert_eq!(priorities.is_empty(), false);
        assert_eq!(priorities.contains(&account), true);
        assert!(priorities.get(&account).is_some());
    }

    #[test]
    fn insert_two() {
        let mut priorities = OrderedPriorities::default();
        priorities.insert(PriorityEntry::new(42, Account::from(1), Priority::new(2.5)));
        priorities.insert(PriorityEntry::new(43, Account::from(2), Priority::new(3.5)));
        assert_eq!(priorities.len(), 2);
        assert_eq!(priorities.is_empty(), false);
        assert_eq!(priorities.contains(&Account::from(1)), true);
        assert_eq!(priorities.contains(&Account::from(2)), true);
    }

    #[test]
    fn dont_insert_when_id_already_taken() {}
}
