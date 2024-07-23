use super::{
    builder::OnlineRepsBuilder,
    online_container::OnlineContainer,
    peered_container::{InsertResult, PeeredContainer},
    Representative,
};
use crate::transport::ChannelEnum;
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
use std::{cmp::max, mem::size_of, sync::Arc, time::Duration};

const ONLINE_WEIGHT_QUORUM: u8 = 67;

pub struct OnlineReps {
    rep_weights: Arc<RepWeightCache>,
    reps: OnlineContainer,
    peered_reps: PeeredContainer,
    trended: Amount,
    online: Amount,
    weight_period: Duration,
    online_weight_minimum: Amount,
}

impl OnlineReps {
    pub(super) fn new(
        rep_weights: Arc<RepWeightCache>,
        weight_period: Duration,
        online_weight_minimum: Amount,
    ) -> Self {
        Self {
            rep_weights,
            reps: OnlineContainer::new(),
            peered_reps: PeeredContainer::new(),
            trended: Amount::zero(),
            online: Amount::zero(),
            weight_period,
            online_weight_minimum,
        }
    }

    pub fn builder() -> OnlineRepsBuilder {
        OnlineRepsBuilder::new()
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
    ) -> InsertResult {
        self.peered_reps.update_or_insert(account, channel)
    }

    pub fn last_request_elapsed(&self, channel_id: usize) -> Option<Duration> {
        self.peered_reps
            .iter_by_channel(channel_id)
            .next()
            .map(|rep| rep.last_request.elapsed())
    }

    /// Query if a peer manages a principle representative
    pub fn is_pr(&self, channel_id: usize) -> bool {
        let min_weight = self.minimum_principal_weight();
        self.peered_reps
            .accounts_by_channel(channel_id)
            .any(|account| self.rep_weights.weight(account) >= min_weight)
    }

    /// Get total available weight from peered representatives
    pub fn total_weight(&self) -> Amount {
        let mut result = Amount::zero();
        let weights = self.rep_weights.read();
        for account in self.peered_reps.accounts() {
            result += weights.get(account).cloned().unwrap_or_default();
        }
        result
    }

    pub fn on_rep_request(&mut self, channel_id: usize) {
        // Find and update the timestamp on all reps available on the endpoint (a single host may have multiple reps)
        self.peered_reps.modify_by_channel(channel_id, |rep| {
            rep.last_request = Instant::now();
        });
    }

    pub fn remove_peered(&mut self, channel_id: usize) -> Vec<Account> {
        self.peered_reps.remove(channel_id)
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
        for rep in self.peered_reps.iter() {
            let weight = self.rep_weights.weight(&rep.account);
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

    /// Total number of peered representatives
    pub fn representatives_count(&self) -> usize {
        self.peered_reps.len()
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

    pub fn count(&self) -> usize {
        self.reps.len()
    }

    pub fn item_size() -> usize {
        OnlineContainer::item_size()
    }

    pub const ELEMENT_SIZE: usize = size_of::<Representative>()
        + size_of::<Account>()
        + size_of::<usize>()
        + size_of::<Account>();

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
