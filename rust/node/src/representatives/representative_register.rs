use super::{online_reps_container::OnlineRepsContainer, Representative};
use crate::{
    stats::{DetailType, Direction, StatType, Stats},
    transport::ChannelEnum,
};
#[cfg(test)]
use mock_instant::Instant;
use primitive_types::U256;
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    Account, Amount,
};
use rsnano_ledger::RepWeightCache;
#[cfg(not(test))]
use std::time::Instant;
use std::{
    cmp::max, collections::HashMap, mem::size_of, net::SocketAddrV6, sync::Arc, time::Duration,
};
use tracing::info;

const ONLINE_WEIGHT_QUORUM: u8 = 67;
pub const DEFAULT_ONLINE_WEIGHT_MINIMUM: Amount = Amount::nano(60_000_000);

pub struct RepresentativeRegister {
    by_account: HashMap<Account, Representative>,
    by_channel_id: HashMap<usize, Vec<Account>>,
    rep_weights: Arc<RepWeightCache>,
    stats: Arc<Stats>,
    reps: OnlineRepsContainer,
    trended: Amount,
    online: Amount,
    weight_period: Duration,
    online_weight_minimum: Amount,
}

pub enum RegisterRepresentativeResult {
    Inserted,
    Updated,
    ChannelChanged(SocketAddrV6),
}

impl RepresentativeRegister {
    pub fn new(rep_weights: Arc<RepWeightCache>, stats: Arc<Stats>) -> Self {
        Self {
            rep_weights,
            stats,
            by_account: HashMap::new(),
            by_channel_id: HashMap::new(),
            reps: OnlineRepsContainer::new(),
            trended: Amount::zero(),
            online: Amount::zero(),
            weight_period: Duration::from_secs(5 * 60),
            online_weight_minimum: DEFAULT_ONLINE_WEIGHT_MINIMUM,
        }
    }

    pub fn builder() -> RepresentativeRegisterBuilder {
        RepresentativeRegisterBuilder {
            stats: None,
            rep_weights: None,
            weight_period: Duration::from_secs(5 * 60),
            online_weight_minimum: DEFAULT_ONLINE_WEIGHT_MINIMUM,
            trended: None,
        }
    }

    pub fn set_weight_period(&mut self, period: Duration) {
        self.weight_period = period;
    }

    pub fn set_online_weight_minimum(&mut self, minimum: Amount) {
        self.online_weight_minimum = minimum;
    }

    pub fn online_weight_minimum(&self) -> Amount {
        self.online_weight_minimum
    }

    pub fn set_online(&mut self, amount: Amount) {
        self.online = amount;
    }

    /** Returns the trended online stake */
    pub fn trended_weight(&self) -> Amount {
        self.trended
    }

    pub fn set_trended(&mut self, trended: Amount) {
        self.trended = trended;
    }

    /** Returns the current online stake */
    pub fn online_weight(&self) -> Amount {
        self.online
    }

    pub fn minimum_principal_weight(&self) -> Amount {
        self.trended / 1000 // 0.1% of trended online weight
    }

    /** Add voting account rep_account to the set of online representatives */
    pub fn observe(&mut self, rep_account: Account) {
        if self.rep_weights.weight(&rep_account) > Amount::zero() {
            let new_insert = self.reps.insert(rep_account, Instant::now());
            let trimmed = self.reps.trim(self.weight_period);

            if new_insert || trimmed {
                self.calculate_online();
            }
        }
    }

    fn calculate_online(&mut self) {
        let mut current = Amount::zero();
        for account in self.reps.iter() {
            current += self.rep_weights.weight(account);
        }
        self.online = current;
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
            let min_weight = { self.minimum_principal_weight() };
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
        self.representatives_filter(usize::MAX, self.minimum_principal_weight())
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

    /// Returns the quorum required for confirmation
    pub fn quorum_delta(&self) -> Amount {
        // Using a larger container to ensure maximum precision
        let weight = max(max(self.online, self.trended), self.online_weight_minimum);

        let delta =
            U256::from(weight.number()) * U256::from(ONLINE_WEIGHT_QUORUM) / U256::from(100);
        Amount::raw(delta.as_u128())
    }

    /// List of online representatives, both the currently sampling ones and the ones observed in the previous sampling period
    pub fn list_online_reps(&self) -> Vec<Account> {
        self.reps.iter().cloned().collect()
    }

    pub fn quorum_info(&self) -> ConfirmationQuorum {
        ConfirmationQuorum {
            quorum_delta: self.quorum_delta(),
            online_weight_quorum_percent: ONLINE_WEIGHT_QUORUM,
            online_weight_minimum: self.online_weight_minimum(),
            online_weight: self.online_weight(),
            trended_weight: self.trended_weight(),
            peers_weight: self.total_weight(),
            minimum_principal_weight: self.minimum_principal_weight(),
        }
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name.into(),
            vec![ContainerInfoComponent::Leaf(ContainerInfo {
                name: "reps".to_string(),
                count: self.count(),
                sizeof_element: Self::item_size(),
            })],
        )
    }

    pub fn count(&self) -> usize {
        self.reps.len()
    }

    pub fn item_size() -> usize {
        OnlineRepsContainer::item_size()
    }

    pub const ELEMENT_SIZE: usize = size_of::<Representative>()
        + size_of::<Account>()
        + size_of::<usize>()
        + size_of::<Account>();
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

pub struct RepresentativeRegisterBuilder {
    stats: Option<Arc<Stats>>,
    rep_weights: Option<Arc<RepWeightCache>>,
    weight_period: Duration,
    online_weight_minimum: Amount,
    trended: Option<Amount>,
}

impl RepresentativeRegisterBuilder {
    pub fn stats(mut self, stats: Arc<Stats>) -> Self {
        self.stats = Some(stats);
        self
    }

    pub fn rep_weights(mut self, weights: Arc<RepWeightCache>) -> Self {
        self.rep_weights = Some(weights);
        self
    }

    pub fn weight_period(mut self, period: Duration) -> Self {
        self.weight_period = period;
        self
    }

    pub fn online_weight_minimum(mut self, minimum: Amount) -> Self {
        self.online_weight_minimum = minimum;
        self
    }

    pub fn trended(mut self, trended: Amount) -> Self {
        self.trended = Some(trended);
        self
    }

    pub fn finish(self) -> RepresentativeRegister {
        let stats = self.stats.unwrap_or_else(|| Arc::new(Stats::default()));
        let rep_weights = self
            .rep_weights
            .unwrap_or_else(|| Arc::new(RepWeightCache::new()));

        let mut register = RepresentativeRegister::new(rep_weights, stats);
        register.set_weight_period(self.weight_period);
        register.set_online_weight_minimum(self.online_weight_minimum);
        if let Some(trended) = self.trended {
            register.set_trended(trended);
        }
        register
    }
}
