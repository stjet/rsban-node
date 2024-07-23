use super::Representative;
use crate::transport::ChannelEnum;
use rsnano_core::Account;
use std::{collections::HashMap, mem::size_of, net::SocketAddrV6, sync::Arc};

#[cfg(test)]
use mock_instant::Instant;
#[cfg(not(test))]
use std::time::Instant;

pub enum InsertResult {
    Inserted,
    Updated,
    ChannelChanged(SocketAddrV6),
}

/// Collection of all representatives that we have a direct connection to
pub(super) struct PeeredContainer {
    by_account: HashMap<Account, Representative>,
    by_channel_id: HashMap<usize, Vec<Account>>,
}

impl PeeredContainer {
    pub const ELEMENT_SIZE: usize = size_of::<Representative>()
        + size_of::<Account>()
        + size_of::<usize>()
        + size_of::<Account>();

    pub fn new() -> Self {
        Self {
            by_account: HashMap::new(),
            by_channel_id: HashMap::new(),
        }
    }

    pub fn update_or_insert(
        &mut self,
        account: Account,
        channel: Arc<ChannelEnum>,
    ) -> InsertResult {
        if let Some(rep) = self.by_account.get_mut(&account) {
            rep.last_response = Instant::now();

            // Update if representative channel was changed
            if rep.channel.remote_endpoint() != channel.remote_endpoint() {
                let new_channel_id = channel.channel_id();
                let old_channel = std::mem::replace(&mut rep.channel, channel);
                if old_channel.channel_id() != new_channel_id {
                    self.remove_channel_id(&account, old_channel.channel_id());
                    self.by_channel_id
                        .entry(new_channel_id)
                        .or_default()
                        .push(account);
                }
                InsertResult::ChannelChanged(old_channel.remote_endpoint())
            } else {
                InsertResult::Updated
            }
        } else {
            let channel_id = channel.channel_id();
            self.by_account
                .insert(account, Representative::new(account, channel));

            let by_id = self.by_channel_id.entry(channel_id).or_default();

            by_id.push(account);
            InsertResult::Inserted
        }
    }

    fn remove_channel_id(&mut self, account: &Account, channel_id: usize) {
        let accounts = self.by_channel_id.get_mut(&channel_id).unwrap();

        if accounts.len() == 1 {
            self.by_channel_id.remove(&channel_id);
        } else {
            accounts.retain(|acc| acc != account);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Representative> {
        self.by_account.values()
    }

    pub fn iter_by_channel(&self, channel_id: usize) -> impl Iterator<Item = &Representative> {
        self.accounts_by_channel(channel_id)
            .map(|account| self.by_account.get(account).unwrap())
    }

    pub fn accounts_by_channel(&self, channel_id: usize) -> impl Iterator<Item = &Account> {
        self.by_channel_id.get(&channel_id).into_iter().flatten()
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Account> {
        self.by_account.keys()
    }

    pub fn modify_by_channel(
        &mut self,
        channel_id: usize,
        mut modify: impl FnMut(&mut Representative),
    ) {
        if let Some(rep_accounts) = self.by_channel_id.get(&channel_id) {
            for rep in rep_accounts {
                modify(self.by_account.get_mut(rep).unwrap());
            }
        }
    }

    pub fn len(&self) -> usize {
        self.by_account.len()
    }

    pub fn remove(&mut self, channel_id: usize) -> Vec<Account> {
        let Some(accounts) = self.by_channel_id.remove(&channel_id) else {
            return Vec::new();
        };
        for account in &accounts {
            self.by_account.remove(account);
        }
        accounts
    }
}
