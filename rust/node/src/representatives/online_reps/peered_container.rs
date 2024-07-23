use super::PeeredRep;
use crate::transport::ChannelId;
use rsnano_core::Account;
use std::{collections::HashMap, mem::size_of, time::Duration};

pub enum InsertResult {
    Inserted,
    Updated,
    /// Returns the old channel id
    ChannelChanged(ChannelId),
}

/// Collection of all representatives that we have a direct connection to
pub(super) struct PeeredContainer {
    by_account: HashMap<Account, PeeredRep>,
    by_channel_id: HashMap<ChannelId, Vec<Account>>,
}

impl PeeredContainer {
    pub const ELEMENT_SIZE: usize =
        size_of::<PeeredRep>() + size_of::<Account>() + size_of::<usize>() + size_of::<Account>();

    pub fn new() -> Self {
        Self {
            by_account: HashMap::new(),
            by_channel_id: HashMap::new(),
        }
    }

    pub fn update_or_insert(
        &mut self,
        account: Account,
        channel_id: ChannelId,
        now: Duration,
    ) -> InsertResult {
        if let Some(rep) = self.by_account.get_mut(&account) {
            // Update if representative channel was changed
            if rep.channel_id != channel_id {
                let old_channel_id = rep.channel_id;
                let new_channel_id = channel_id;
                rep.channel_id = new_channel_id;
                self.remove_channel_id(&account, old_channel_id);
                self.by_channel_id
                    .entry(new_channel_id)
                    .or_default()
                    .push(account);
                InsertResult::ChannelChanged(old_channel_id)
            } else {
                InsertResult::Updated
            }
        } else {
            self.by_account
                .insert(account, PeeredRep::new(account, channel_id, now));

            let by_id = self.by_channel_id.entry(channel_id).or_default();
            by_id.push(account);
            InsertResult::Inserted
        }
    }

    fn remove_channel_id(&mut self, account: &Account, channel_id: ChannelId) {
        let accounts = self.by_channel_id.get_mut(&channel_id).unwrap();

        if accounts.len() == 1 {
            self.by_channel_id.remove(&channel_id);
        } else {
            accounts.retain(|acc| acc != account);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &PeeredRep> {
        self.by_account.values()
    }

    pub fn iter_by_channel(&self, channel_id: ChannelId) -> impl Iterator<Item = &PeeredRep> {
        self.accounts_by_channel(channel_id)
            .map(|account| self.by_account.get(account).unwrap())
    }

    pub fn accounts_by_channel(&self, channel_id: ChannelId) -> impl Iterator<Item = &Account> {
        self.by_channel_id.get(&channel_id).into_iter().flatten()
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Account> {
        self.by_account.keys()
    }

    pub fn modify_by_channel(
        &mut self,
        channel_id: ChannelId,
        mut modify: impl FnMut(&mut PeeredRep),
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

    pub fn remove(&mut self, channel_id: ChannelId) -> Vec<Account> {
        let Some(accounts) = self.by_channel_id.remove(&channel_id) else {
            return Vec::new();
        };
        for account in &accounts {
            self.by_account.remove(account);
        }
        accounts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let container = PeeredContainer::new();
        assert_eq!(container.len(), 0);
        assert_eq!(container.iter().count(), 0);
        assert_eq!(container.iter_by_channel(42.into()).count(), 0);
        assert_eq!(container.accounts_by_channel(42.into()).count(), 0);
        assert_eq!(container.accounts().count(), 0);
    }

    #[test]
    fn insert() {
        let mut container = PeeredContainer::new();
        let account = Account::from(1);
        let channel_id = ChannelId::from(2);
        let now = Duration::from_secs(3);
        container.update_or_insert(account, channel_id, now);

        assert_eq!(
            container.iter().cloned().collect::<Vec<_>>(),
            vec![PeeredRep::new(account, channel_id, now)]
        );
    }
}
