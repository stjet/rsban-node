use rsnano_core::{Account, Amount};
use rsnano_ledger::Ledger;

use super::Representative;
use crate::{transport::ChannelEnum, OnlineReps};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::SystemTime,
};

pub struct RepresentativeRegister {
    by_account: HashMap<Account, Representative>,
    by_channel_id: HashMap<usize, Vec<Account>>,
    ledger: Arc<Ledger>,
    online_reps: Arc<Mutex<OnlineReps>>,
}

pub enum RegisterRepresentativeResult {
    Inserted,
    Updated,
    ChannelChanged(Arc<ChannelEnum>),
}

impl RepresentativeRegister {
    pub fn new(ledger: Arc<Ledger>, online_reps: Arc<Mutex<OnlineReps>>) -> Self {
        Self {
            ledger,
            online_reps,
            by_account: HashMap::new(),
            by_channel_id: HashMap::new(),
        }
    }

    /// Returns the old channel if the representative was already in the collection
    pub fn update_or_insert(
        &mut self,
        account: Account,
        channel: Arc<ChannelEnum>,
    ) -> RegisterRepresentativeResult {
        if let Some(rep) = self.by_account.get_mut(&account) {
            rep.set_last_response(SystemTime::now());
            if rep.channel().remote_endpoint() != channel.remote_endpoint() {
                let old_channel = rep.set_channel(channel);
                RegisterRepresentativeResult::ChannelChanged(old_channel)
            } else {
                RegisterRepresentativeResult::Updated
            }
        } else {
            let channel_id = channel.channel_id();
            self.by_account
                .insert(account, Representative::new(account, channel));

            let by_id = self.by_channel_id.entry(channel_id).or_default();

            by_id.push(account);
            RegisterRepresentativeResult::Inserted
        }
    }

    pub fn is_pr(&self, channel: &ChannelEnum) -> bool {
        if let Some(existing) = self.by_channel_id.get(&channel.channel_id()) {
            let min_weight = {
                let guard = self.online_reps.lock().unwrap();
                guard.minimum_principal_weight()
            };
            existing
                .iter()
                .any(|account| self.ledger.weight(account) > min_weight)
        } else {
            false
        }
    }

    pub fn total_weight(&self) -> Amount {
        let mut result = Amount::zero();
        for (account, rep) in &self.by_account {
            if rep.channel().is_alive() {
                result += self.ledger.weight(account);
            }
        }
        result
    }
}
