use super::{
    ordered_priorities::PriorityEntry,
    priority::{Priority, PriorityKeyDesc},
};
use rsnano_core::{Account, BlockHash};
use std::{
    collections::{BTreeMap, VecDeque},
    mem::size_of,
};

pub(crate) struct BlockingEntry {
    pub dependency: BlockHash,
    pub original_entry: PriorityEntry,
    pub dependency_account: Account,
}

impl BlockingEntry {
    fn priority(&self) -> Priority {
        self.original_entry.priority
    }

    fn account(&self) -> &Account {
        &self.original_entry.account
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
        let account = entry.account().clone();
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

    pub fn count_by_dependency_account(&self, dep_account: &Account) -> usize {
        self.by_dependency_account
            .get(dep_account)
            .map(|accs| accs.len())
            .unwrap_or_default()
    }

    pub fn next(&self, filter: impl Fn(&BlockHash) -> bool) -> Option<BlockHash> {
        // Scan all entries with unknown dependency account
        let accounts = self.by_dependency_account.get(&Account::zero())?;
        accounts
            .iter()
            .map(|a| self.by_account.get(a).unwrap())
            .find(|e| filter(&e.dependency))
            .map(|e| e.dependency)
    }

    pub fn iter_start_dep_account(&self, start: Account) -> impl Iterator<Item = &BlockingEntry> {
        self.by_dependency_account
            .range(start..)
            .flat_map(|(_, accs)| accs)
            .map(|acc| self.by_account.get(acc).unwrap())
    }

    pub fn get(&self, account: &Account) -> Option<&BlockingEntry> {
        self.by_account.get(account)
    }

    pub fn remove(&mut self, account: &Account) -> Option<BlockingEntry> {
        let entry = self.by_account.remove(account)?;
        self.remove_indexes(&entry);
        Some(entry)
    }

    pub fn pop_front(&mut self) -> Option<BlockingEntry> {
        let account = self.sequenced.pop_front()?;
        self.remove(&account)
    }

    pub fn modify_dependency_account(
        &mut self,
        dependency: &BlockHash,
        new_dependency_account: Account,
    ) -> usize {
        let Some(accounts) = self.by_dependency.get(dependency) else {
            return 0;
        };

        let mut updated = 0;

        for account in accounts {
            let entry = self.by_account.get_mut(account).unwrap();
            if entry.dependency_account != new_dependency_account {
                let old_dependency_account = entry.dependency_account;
                entry.dependency_account = new_dependency_account;
                let old = self
                    .by_dependency_account
                    .get_mut(&old_dependency_account)
                    .unwrap();
                if old.len() == 1 {
                    self.by_dependency_account.remove(&old_dependency_account);
                } else {
                    old.retain(|a| a != entry.account());
                }
                self.by_dependency_account
                    .entry(new_dependency_account)
                    .or_default()
                    .push(*entry.account());

                updated += 1;
            }
        }

        updated
    }

    fn remove_indexes(&mut self, entry: &BlockingEntry) {
        self.sequenced.retain(|i| i != entry.account());
        let accounts = self.by_priority.get_mut(&entry.priority().into()).unwrap();
        if accounts.len() > 1 {
            accounts.retain(|i| i != entry.account());
        } else {
            self.by_priority.remove(&entry.priority().into());
        }
        let accounts = self.by_dependency.get_mut(&entry.dependency).unwrap();
        if accounts.len() > 1 {
            accounts.retain(|i| i != entry.account());
        } else {
            self.by_dependency.remove(&entry.dependency);
        }
        let accounts = self
            .by_dependency_account
            .get_mut(&entry.dependency_account)
            .unwrap();
        if accounts.len() > 1 {
            accounts.retain(|i| i != entry.account());
        } else {
            self.by_dependency_account.remove(&entry.dependency_account);
        }
    }
}
