use super::ordered_priorities::PriorityEntry;
use ordered_float::OrderedFloat;
use rsnano_core::{Account, BlockHash};
use std::collections::{BTreeMap, VecDeque};

pub(crate) struct BlockingEntry {
    account: Account,
    dependency: BlockHash,
    original_entry: PriorityEntry,
}

impl BlockingEntry {
    fn priority(&self) -> OrderedFloat<f32> {
        self.original_entry.priority
    }
}

/// A blocked account is an account that has failed to insert a new block because the source block is not currently present in the ledger
/// An account is unblocked once it has a block successfully inserted
#[derive(Default)]
pub(crate) struct OrderedBlocking {
    by_account: BTreeMap<Account, BlockingEntry>,
    sequenced: VecDeque<Account>,
    by_priority: BTreeMap<OrderedFloat<f32>, Vec<Account>>,
}

impl OrderedBlocking {
    pub fn insert(&mut self, entry: BlockingEntry) -> bool {
        let account = entry.account;
        let prio = entry.priority();
        if self.by_account.contains_key(&account) {
            return false;
        }

        self.by_account.insert(account, entry);
        self.sequenced.push_back(account);
        self.by_priority.entry(prio).or_default().push(account);
        true
    }

    pub fn contains(&self, account: &Account) -> bool {
        self.by_account.contains_key(account)
    }
}
