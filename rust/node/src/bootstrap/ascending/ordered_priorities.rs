use ordered_float::OrderedFloat;
use rand::{thread_rng, RngCore};
use rsnano_core::Account;
use std::collections::VecDeque;
use std::mem::size_of;
use std::ops::{Add, Bound, Deref, Div, Mul, Sub};
use std::{collections::BTreeMap, time::Instant};

#[derive(Clone, Default)]
pub(crate) struct PriorityEntry {
    pub account: Account,
    pub priority: Priority,
    pub timestamp: Option<Instant>,
    pub id: u64, // Uniformly distributed, used for random querying
}

impl PriorityEntry {
    pub fn new(account: Account, priority: Priority) -> Self {
        Self {
            account,
            priority,
            timestamp: None,
            id: thread_rng().next_u64(),
        }
    }
}

#[derive(PartialEq, Eq, Default, Clone, Copy, Ord, PartialOrd)]
pub struct Priority(OrderedFloat<f64>);

impl Priority {
    pub const fn new(value: f64) -> Self {
        Self(OrderedFloat(value))
    }

    pub const ZERO: Self = Self(OrderedFloat(0.0));
}

impl Add for Priority {
    type Output = Priority;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Priority {
    type Output = Priority;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul<f64> for Priority {
    type Output = Priority;

    fn mul(self, rhs: f64) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div<f64> for Priority {
    type Output = Priority;

    fn div(self, rhs: f64) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl From<Priority> for f64 {
    fn from(value: Priority) -> Self {
        value.0 .0
    }
}

impl std::fmt::Debug for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0 .0, f)
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0 .0, f)
    }
}

#[derive(PartialEq, Eq, Default, Clone, Copy)]
pub(crate) struct PriorityKeyDesc(pub Priority);

impl Ord for PriorityKeyDesc {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // order descending
        other.0.cmp(&self.0)
    }
}

impl PartialOrd for PriorityKeyDesc {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Deref for PriorityKeyDesc {
    type Target = Priority;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Priority> for PriorityKeyDesc {
    fn from(value: Priority) -> Self {
        Self(value)
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

    pub fn pop_lowest_priority(&mut self) -> Option<PriorityEntry> {
        if let Some(mut entry) = self.by_priority.last_entry() {
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
                let Some(ts) = entry.timestamp else {
                    return false;
                };
                if ts > cutoff {
                    return false;
                }
                filter(&entry.account)
            })
            .map(|e| e.account)
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
