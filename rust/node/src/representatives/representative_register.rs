use super::Representative;
use crate::{
    stats::{DetailType, Direction, StatType, Stats},
    transport::ChannelEnum,
    OnlineReps, ONLINE_WEIGHT_QUORUM,
};
use rsnano_core::{Account, Amount};
use rsnano_ledger::RepWeightCache;
use rsnano_messages::ProtocolInfo;
use std::{
    collections::HashMap,
    mem::size_of,
    net::SocketAddrV6,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::info;

pub struct RepresentativeRegister {
    by_account: HashMap<Account, Representative>,
    by_channel_id: HashMap<usize, Vec<Account>>,
    rep_weights: Arc<RepWeightCache>,
    online_reps: Arc<Mutex<OnlineReps>>,
    protocol_info: ProtocolInfo,
    stats: Arc<Stats>,
}

pub enum RegisterRepresentativeResult {
    Inserted,
    Updated,
    ChannelChanged(SocketAddrV6),
}

impl RepresentativeRegister {
    pub const ELEMENT_SIZE: usize = size_of::<Representative>()
        + size_of::<Account>()
        + size_of::<usize>()
        + size_of::<Account>();

    pub fn new(
        rep_weights: Arc<RepWeightCache>,
        online_reps: Arc<Mutex<OnlineReps>>,
        stats: Arc<Stats>,
        protocol_info: ProtocolInfo,
    ) -> Self {
        Self {
            rep_weights,
            online_reps,
            stats,
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

    pub fn last_request_elapsed(&self, channel_id: usize) -> Option<Duration> {
        self.by_channel_id.get(&channel_id).map(|i| {
            self.by_account
                .get(i.first().unwrap())
                .unwrap()
                .last_request
                .elapsed()
        })
    }

    /// Query if a peer manages a principle representative
    pub fn is_pr(&self, channel_id: usize) -> bool {
        if let Some(existing) = self.by_channel_id.get(&channel_id) {
            let min_weight = {
                let guard = self.online_reps.lock().unwrap();
                guard.minimum_principal_weight()
            };
            existing
                .iter()
                .any(|account| self.rep_weights.weight(account) >= min_weight)
        } else {
            false
        }
    }

    /// Get total available weight from representatives
    pub fn total_weight(&self) -> Amount {
        let mut result = Amount::zero();
        let weights = self.rep_weights.read();
        for (account, _) in &self.by_account {
            result += weights.get(account).cloned().unwrap_or_default();
        }
        result
    }

    pub fn on_rep_request(&mut self, channel_id: usize) {
        // Find and update the timestamp on all reps available on the endpoint (a single host may have multiple reps)
        if let Some(rep_accounts) = self.by_channel_id.get(&channel_id) {
            for rep in rep_accounts {
                self.by_account.get_mut(rep).unwrap().last_request = Instant::now();
            }
        }
    }

    pub fn evict(&mut self, channel_ids: &[usize]) {
        let mut to_delete = Vec::new();

        for channel_id in channel_ids {
            if let Some(accounts) = self.by_channel_id.get(&channel_id) {
                for account in accounts {
                    to_delete.push((*account, *channel_id));
                }
            }
        }
        for (account, channel_id) in to_delete {
            let rep = self.by_account.remove(&account).unwrap();
            self.remove_channel_id(&account, channel_id);
            info!(
                "Evicting representative {} with dead channel at {}",
                account.encode_account(),
                rep.channel.remote_endpoint()
            );
            self.stats
                .inc_dir(StatType::RepCrawler, DetailType::ChannelDead, Direction::In);
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

    /// Request a list of the top \p count known representatives in descending order of weight, with at least \p weight_a voting weight, and optionally with a minimum version \p minimum_protocol_version
    pub fn representatives(&self) -> Vec<Representative> {
        self.representatives_filter(usize::MAX, Amount::zero(), None)
    }

    /// Request a list of the top \p count known principal representatives in descending order of weight, optionally with a minimum version \p minimum_protocol_version
    pub fn principal_representatives(&self) -> Vec<Representative> {
        self.representatives_filter(
            usize::MAX,
            self.online_reps.lock().unwrap().minimum_principal_weight(),
            None,
        )
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
            let weight = self.rep_weights.weight(account);
            if weight > min_weight && rep.channel.network_version() >= min_protocol_version {
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

    /// Total number of representatives
    pub fn representatives_count(&self) -> usize {
        self.by_account.len()
    }

    pub fn quorum_info(&self) -> ConfirmationQuorum {
        let online = self.online_reps.lock().unwrap();
        ConfirmationQuorum {
            quorum_delta: online.delta(),
            online_weight_quorum_percent: ONLINE_WEIGHT_QUORUM,
            online_weight_minimum: online.online_weight_minimum(),
            online_weight: online.online(),
            trended_weight: online.trended(),
            peers_weight: self.total_weight(),
        }
    }
}

pub struct ConfirmationQuorum {
    pub quorum_delta: Amount,
    pub online_weight_quorum_percent: u8,
    pub online_weight_minimum: Amount,
    pub online_weight: Amount,
    pub trended_weight: Amount,
    pub peers_weight: Amount,
}
