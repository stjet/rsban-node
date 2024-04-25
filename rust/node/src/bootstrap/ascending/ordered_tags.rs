use rsnano_core::{Account, HashOrAccount};
use std::{collections::HashMap, time::Duration};

#[derive(Default, PartialEq, Eq, Debug)]
pub(crate) enum QueryType {
    #[default]
    Invalid,
    BlocksByHash,
    BlocksByAccount,
    // TODO: account_info
}

#[derive(Default)]
pub(crate) struct AsyncTag {
    pub query_type: QueryType,
    pub id: u64,
    pub start: HashOrAccount,
    pub time: Duration,
    pub account: Account,
}

pub(crate) struct OrderedTags {
    by_id: HashMap<u64, AsyncTag>,
    by_account: HashMap<Account, Vec<u64>>,
    sequenced: Vec<u64>,
}

impl OrderedTags {
    pub(crate) fn len(&self) -> usize {
        self.sequenced.len()
    }

    pub(crate) fn insert(&mut self, tag: AsyncTag) {
        let id = tag.id;
        let account = tag.account;
        if let Some(old) = self.by_id.insert(id, tag) {
            self.remove_internal(old.id, &old.account);
        }
        self.by_account.entry(account).or_default().push(id);
        self.sequenced.push(id);
    }

    fn remove_internal(&mut self, id: u64, account: &Account) {
        self.by_id.remove(&id);
        if let Some(ids) = self.by_account.get_mut(account) {
            if ids.len() == 1 {
                self.by_account.remove(account);
            } else {
                ids.retain(|i| *i != id)
            }
        }
        self.sequenced.retain(|i| *i != id);
    }
}
