use super::Representative;
use crate::{
    stats::{DetailType, Direction, StatType, Stats},
    transport::ChannelEnum,
    OnlineReps, ONLINE_WEIGHT_QUORUM,
};
use rsnano_core::{utils::ContainerInfoComponent, Account, Amount};
use rsnano_ledger::RepWeightCache;
use std::{
    collections::HashMap,
    mem::size_of,
    net::SocketAddrV6,
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::info;

pub struct RepresentativeRegister {
    by_account: HashMap<Account, Representative>,
    by_channel_id: HashMap<usize, Vec<Account>>,
    rep_weights: Arc<RepWeightCache>,
    online_reps: OnlineReps,
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
        online_reps: OnlineReps,
        stats: Arc<Stats>,
    ) -> Self {
        Self {
            rep_weights,
            online_reps,
            stats,
            by_account: HashMap::new(),
            by_channel_id: HashMap::new(),
        }
    }

    pub fn observe(&mut self, rep_account: Account) {
        self.online_reps.observe(rep_account);
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
            let min_weight = { self.online_reps.minimum_principal_weight() };
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
        self.representatives_filter(usize::MAX, Amount::zero())
    }

    /// Request a list of the top \p count known principal representatives in descending order of weight, optionally with a minimum version \p minimum_protocol_version
    pub fn principal_representatives(&self) -> Vec<Representative> {
        self.representatives_filter(usize::MAX, self.online_reps.minimum_principal_weight())
    }

    /// Request a list of the top **max_results** known representatives in descending order
    /// of weight, with at least **weight** voting weight, and optionally with a
    /// minimum version **min_protocol_version**
    pub fn representatives_filter(
        &self,
        max_results: usize,
        min_weight: Amount,
    ) -> Vec<Representative> {
        let mut reps_with_weight = Vec::new();
        for (account, rep) in &self.by_account {
            let weight = self.rep_weights.weight(account);
            if weight > min_weight {
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

    pub fn trended_weight(&self) -> Amount {
        self.online_reps.trended()
    }

    pub fn quorum_delta(&self) -> Amount {
        self.online_reps.delta()
    }

    pub fn minimum_principal_weight(&self) -> Amount {
        self.online_reps.minimum_principal_weight()
    }

    pub fn set_online(&mut self, amount: Amount) {
        self.online_reps.set_online(amount)
    }

    pub fn list_online_reps(&self) -> Vec<Account> {
        self.online_reps.list()
    }

    pub fn set_trended(&mut self, trended: Amount) {
        self.online_reps.set_trended(trended);
    }

    pub fn quorum_info(&self) -> ConfirmationQuorum {
        ConfirmationQuorum {
            quorum_delta: self.online_reps.delta(),
            online_weight_quorum_percent: ONLINE_WEIGHT_QUORUM,
            online_weight_minimum: self.online_reps.online_weight_minimum(),
            online_weight: self.online_reps.online(),
            trended_weight: self.online_reps.trended(),
            peers_weight: self.total_weight(),
            minimum_principal_weight: self.online_reps.minimum_principal_weight(),
        }
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        self.online_reps.collect_container_info(name)
    }
}

pub struct ConfirmationQuorum {
    pub quorum_delta: Amount,
    pub online_weight_quorum_percent: u8,
    pub online_weight_minimum: Amount,
    pub online_weight: Amount,
    pub trended_weight: Amount,
    pub peers_weight: Amount,
    pub minimum_principal_weight: Amount,
}
