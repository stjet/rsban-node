use super::ordered_priorities::{Priority, PriorityEntry, PriorityKeyDesc};
use rsnano_core::{Account, BlockHash};
use std::{
    collections::{BTreeMap, VecDeque},
    mem::size_of,
};

pub(crate) struct BlockingEntry {
    pub account: Account,
    pub dependency: BlockHash,
    pub original_entry: PriorityEntry,
    pub dependency_account: Account,
}

impl BlockingEntry {
    fn priority(&self) -> Priority {
        self.original_entry.priority
    }
}

/// A blocked account is an account that has failed to insert a new block because the source block is not currently present in the ledger
/// An account is unblocked once it has a block successfully inserted
#[derive(Default)]
pub(crate) struct OrderedBlocking {
    by_account: BTreeMap<Account, BlockingEntry>,
    sequenced: VecDeque<Account>,
    // descending
    by_priority: BTreeMap<PriorityKeyDesc, VecDeque<Account>>,
    by_dependency: BTreeMap<BlockHash, Vec<Account>>,
    by_dependency_account: BTreeMap<Account, Vec<Account>>,
}

impl OrderedBlocking {
    pub const ELEMENT_SIZE: usize =
        size_of::<BlockingEntry>() + size_of::<Account>() * 3 + size_of::<f32>();

    pub fn len(&self) -> usize {
        self.sequenced.len()
    }

    pub fn insert(&mut self, entry: BlockingEntry) -> bool {
        let account = entry.account;
        let prio = entry.priority();
        let dependency = entry.dependency;
        let dependency_account = entry.dependency_account;
        if self.by_account.contains_key(&account) {
            return false;
        }

        self.by_account.insert(account, entry);
        self.sequenced.push_back(account);
        self.by_priority
            .entry(prio.into())
            .or_default()
            .push_back(account);
        self.by_dependency
            .entry(dependency)
            .or_default()
            .push(account);
        self.by_dependency_account
            .entry(dependency_account)
            .or_default()
            .push(account);
        true
    }

    pub fn contains(&self, account: &Account) -> bool {
        self.by_account.contains_key(account)
    }

    pub fn get(&self, account: &Account) -> Option<&BlockingEntry> {
        self.by_account.get(account)
    }

    pub fn remove(&mut self, account: &Account) {
        if let Some(entry) = self.by_account.remove(account) {
            self.remove_indexes(&entry);
        }
    }

    pub fn pop_lowest_priority(&mut self) -> Option<BlockingEntry> {
        if let Some((_, accounts)) = self.by_priority.last_key_value() {
            let account = accounts[0];
            let result = self.by_account.remove(&account).unwrap();
            self.remove_indexes(&result);
            Some(result)
        } else {
            None
        }
    }

    fn remove_indexes(&mut self, entry: &BlockingEntry) {
        self.sequenced.retain(|i| *i != entry.account);
        let accounts = self.by_priority.get_mut(&entry.priority().into()).unwrap();
        if accounts.len() > 1 {
            accounts.retain(|i| *i != entry.account);
        } else {
            self.by_priority.remove(&entry.priority().into());
        }
        let accounts = self.by_dependency.get_mut(&entry.dependency).unwrap();
        if accounts.len() > 1 {
            accounts.retain(|i| *i != entry.account);
        } else {
            self.by_dependency.remove(&entry.dependency);
        }
        let accounts = self
            .by_dependency_account
            .get_mut(&entry.dependency_account)
            .unwrap();
        if accounts.len() > 1 {
            accounts.retain(|i| *i != entry.account);
        } else {
            self.by_dependency_account.remove(&entry.dependency_account);
        }
    }
}
