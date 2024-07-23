mod builder;
mod online_container;
mod peered_container;
mod peered_rep;

pub use builder::{OnlineRepsBuilder, DEFAULT_ONLINE_WEIGHT_MINIMUM};
pub use peered_container::InsertResult;
pub use peered_rep::PeeredRep;

use crate::transport::{ChannelEnum, ChannelId};
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
use std::{cmp::max, sync::Arc, time::Duration};
use {online_container::OnlineContainer, peered_container::PeeredContainer};

const ONLINE_WEIGHT_QUORUM: u8 = 67;

/// Keeps track of all representatives that are online
/// and all representatives to which we have a direct connection
pub struct OnlineReps {
    rep_weights: Arc<RepWeightCache>,
    online_reps: OnlineContainer,
    peered_reps: PeeredContainer,
    trended_weight: Amount,
    online_weight: Amount,
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
            online_reps: OnlineContainer::new(),
            peered_reps: PeeredContainer::new(),
            trended_weight: Amount::zero(),
            online_weight: Amount::zero(),
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
        self.online_weight = amount;
    }

    /** Returns the trended online stake */
    pub fn trended_weight(&self) -> Amount {
        self.trended_weight
    }

    pub fn set_trended(&mut self, trended: Amount) {
        self.trended_weight = trended;
    }

    /** Returns the current online stake */
    pub fn online_weight(&self) -> Amount {
        self.online_weight
    }

    pub fn minimum_principal_weight(&self) -> Amount {
        self.trended_weight / 1000 // 0.1% of trended online weight
    }

    /// Add voting account rep_account to the set of online representatives
    pub fn vote_observed(&mut self, rep_account: Account) {
        if self.rep_weights.weight(&rep_account) > Amount::zero() {
            let new_insert = self.online_reps.insert(rep_account, Instant::now());
            let trimmed = self.online_reps.trim(self.weight_period);

            if new_insert || trimmed {
                self.calculate_online_weight();
            }
        }
    }

    /// Add rep_account to the set of peered representatives
    pub fn peer_observed(
        &mut self,
        rep_account: Account,
        channel: Arc<ChannelEnum>,
    ) -> InsertResult {
        self.peered_reps.update_or_insert(rep_account, channel)
    }

    fn calculate_online_weight(&mut self) {
        let mut current = Amount::zero();
        for account in self.online_reps.iter() {
            current += self.rep_weights.weight(account);
        }
        self.online_weight = current;
    }

    /// Query if a peer manages a principle representative
    pub fn is_pr(&self, channel_id: ChannelId) -> bool {
        let min_weight = self.minimum_principal_weight();
        self.peered_reps
            .accounts_by_channel(channel_id)
            .any(|account| self.rep_weights.weight(account) >= min_weight)
    }

    /// Get total available weight from peered representatives
    pub fn peered_weight(&self) -> Amount {
        let mut result = Amount::zero();
        let weights = self.rep_weights.read();
        for account in self.peered_reps.accounts() {
            result += weights.get(account).cloned().unwrap_or_default();
        }
        result
    }

    pub fn on_rep_request(&mut self, channel_id: ChannelId) {
        // Find and update the timestamp on all reps available on the endpoint (a single host may have multiple reps)
        self.peered_reps.modify_by_channel(channel_id, |rep| {
            rep.last_request = Instant::now();
        });
    }

    pub fn last_request_elapsed(&self, channel_id: ChannelId) -> Option<Duration> {
        self.peered_reps
            .iter_by_channel(channel_id)
            .next()
            .map(|rep| rep.last_request.elapsed())
    }

    pub fn remove_peer(&mut self, channel_id: ChannelId) -> Vec<Account> {
        self.peered_reps.remove(channel_id)
    }

    /// Request a list of the top \p count known representatives in descending order of weight, with at least \p weight_a voting weight, and optionally with a minimum version \p minimum_protocol_version
    pub fn peered_reps(&self) -> Vec<PeeredRep> {
        self.representatives_filter(usize::MAX, Amount::zero())
    }

    /// Request a list of the top \p count known principal representatives in descending order of weight, optionally with a minimum version \p minimum_protocol_version
    pub fn peered_principal_reps(&self) -> Vec<PeeredRep> {
        self.representatives_filter(usize::MAX, self.minimum_principal_weight())
    }

    /// Request a list of the top **max_results** known representatives in descending order
    /// of weight, with at least **weight** voting weight, and optionally with a
    /// minimum version **min_protocol_version**
    pub fn representatives_filter(&self, max_results: usize, min_weight: Amount) -> Vec<PeeredRep> {
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
    pub fn peered_reps_count(&self) -> usize {
        self.peered_reps.len()
    }

    /// Returns the quorum required for confirmation
    pub fn quorum_delta(&self) -> Amount {
        // Using a larger container to ensure maximum precision
        let weight = max(
            max(self.online_weight, self.trended_weight),
            self.online_weight_minimum,
        );

        let delta =
            U256::from(weight.number()) * U256::from(ONLINE_WEIGHT_QUORUM) / U256::from(100);
        Amount::raw(delta.as_u128())
    }

    pub fn quorum_percent(&self) -> u8 {
        ONLINE_WEIGHT_QUORUM
    }

    /// List of online representatives, both the currently sampling ones and the ones observed in the previous sampling period
    pub fn online_reps(&self) -> Vec<Account> {
        self.online_reps.iter().cloned().collect()
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name.into(),
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "online".to_string(),
                    count: self.online_reps.len(),
                    sizeof_element: OnlineContainer::ELEMENT_SIZE,
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "peered".to_string(),
                    count: self.peered_reps.len(),
                    sizeof_element: PeeredContainer::ELEMENT_SIZE,
                }),
            ],
        )
    }
}
