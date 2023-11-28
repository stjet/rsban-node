use rsnano_core::{Account, Amount};
use rsnano_ledger::Ledger;
use rsnano_messages::ProtocolInfo;

use super::Representative;
use crate::{transport::ChannelEnum, OnlineReps};
use std::{
    collections::HashMap,
    net::SocketAddrV6,
    sync::{Arc, Mutex},
    time::SystemTime,
};

pub struct RepresentativeRegister {
    by_account: HashMap<Account, Representative>,
    by_channel_id: HashMap<usize, Vec<Account>>,
    ledger: Arc<Ledger>,
    online_reps: Arc<Mutex<OnlineReps>>,
    protocol_info: ProtocolInfo,
}

pub enum RegisterRepresentativeResult {
    Inserted,
    Updated,
    ChannelChanged(SocketAddrV6),
}

impl RepresentativeRegister {
    pub fn new(
        ledger: Arc<Ledger>,
        online_reps: Arc<Mutex<OnlineReps>>,
        protocol_info: ProtocolInfo,
    ) -> Self {
        Self {
            ledger,
            online_reps,
            protocol_info,
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
                let new_channel_id = channel.channel_id();
                let old_channel = rep.set_channel(channel);
                if old_channel.channel_id() != new_channel_id {
                    self.remove_channel_id(&account, old_channel.channel_id());
                    self.by_channel_id
                        .entry(new_channel_id)
                        .or_default()
                        .push(account);
                }
                RegisterRepresentativeResult::ChannelChanged(old_channel.remote_endpoint())
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

    pub fn on_rep_request(&mut self, channel: &ChannelEnum) {
        if !channel.remote_endpoint().ip().is_unspecified() {
            // Find and update the timestamp on all reps available on the endpoint (a single host may have multiple reps)
            if let Some(rep_accounts) = self.by_channel_id.get(&channel.channel_id()) {
                for rep in rep_accounts {
                    self.by_account
                        .get_mut(rep)
                        .unwrap()
                        .set_last_request(SystemTime::now());
                }
            }
        }
    }

    pub fn cleanup_reps(&mut self) {
        let mut to_delete = Vec::new();
        // Check known rep channels
        for (account, rep) in &self.by_account {
            if !rep.channel().is_alive() {
                // Remove reps with closed channels
                to_delete.push((*account, rep.channel().channel_id()));
            }
        }

        for (account, channel_id) in to_delete {
            self.by_account.remove(&account);
            self.remove_channel_id(&account, channel_id);
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

    pub fn representatives(&self) -> Vec<Representative> {
        self.representatives_filter(usize::MAX, Amount::zero(), None)
    }

    /// Request a list of the top **max_results** known representatives in descending order
    /// of weight, with at least **weight** voting weight, and optionally with a
    /// minimum version **min_protocol_version**
    pub fn representatives_filter(
        &self,
        max_results: usize,
        min_weight: Amount,
        min_protocol_version: Option<u8>,
    ) -> Vec<Representative> {
        let min_protocol_version = min_protocol_version.unwrap_or(self.protocol_info.version_min);
        let mut reps_with_weight = Vec::new();
        for (account, rep) in &self.by_account {
            let weight = self.ledger.weight(account);
            if weight > min_weight && rep.channel().network_version() >= min_protocol_version {
                reps_with_weight.push((rep.clone(), weight));
            }
        }

        reps_with_weight.sort_by(|a, b| b.1.cmp(&a.1));

        reps_with_weight
            .drain(..)
            .take(max_results)
            .map(|(rep, _)| rep)
            .collect()
    }

    pub fn representatives_count(&self) -> usize {
        self.by_account.len()
    }
}
