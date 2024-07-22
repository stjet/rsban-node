use primitive_types::U256;
use rsnano_core::utils::{ContainerInfo, ContainerInfoComponent};
use rsnano_core::{Account, Amount};
use rsnano_ledger::RepWeightCache;
use std::time::Duration;
use std::{cmp::max, sync::Arc};

#[cfg(test)]
use mock_instant::Instant;
#[cfg(not(test))]
use std::time::Instant;

use super::online_reps_container::OnlineRepsContainer;

pub const ONLINE_WEIGHT_QUORUM: u8 = 67;

/// Track online representatives and trend online weight
pub struct OnlineReps {
    rep_weights: Arc<RepWeightCache>,
    reps: OnlineRepsContainer,
    trended: Amount,
    online: Amount,
    weight_period: Duration,
    online_weight_minimum: Amount,
}

impl OnlineReps {
    pub fn new(rep_weights: Arc<RepWeightCache>) -> Self {
        Self {
            rep_weights,
            reps: OnlineRepsContainer::new(),
            trended: Amount::zero(),
            online: Amount::zero(),
            weight_period: Duration::from_secs(5 * 60),
            online_weight_minimum: DEFAULT_ONLINE_WEIGHT_MINIMUM,
        }
    }

    pub fn set_weight_period(&mut self, period: Duration) {
        self.weight_period = period;
    }

    pub fn set_online_weight_minimum(&mut self, minimum: Amount) {
        self.online_weight_minimum = minimum;
    }

    pub fn set_online(&mut self, amount: Amount) {
        self.online = amount;
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

    /** Returns the trended online stake */
    pub fn trended(&self) -> Amount {
        self.trended
    }

    pub fn set_trended(&mut self, trended: Amount) {
        self.trended = trended;
    }

    /** Returns the current online stake */
    pub fn online(&self) -> Amount {
        self.online
    }

    pub fn minimum_principal_weight(&self) -> Amount {
        self.trended / 1000 // 0.1% of trended online weight
    }

    /** Returns the quorum required for confirmation*/
    pub fn delta(&self) -> Amount {
        // Using a larger container to ensure maximum precision
        let weight = max(max(self.online, self.trended), self.online_weight_minimum);

        let delta =
            U256::from(weight.number()) * U256::from(ONLINE_WEIGHT_QUORUM) / U256::from(100);
        Amount::raw(delta.as_u128())
    }

    /** List of online representatives, both the currently sampling ones and the ones observed in the previous sampling period */
    pub fn list(&self) -> Vec<Account> {
        self.reps.iter().cloned().collect()
    }

    pub fn clear(&mut self) {
        self.reps.clear();
        self.online = Amount::zero();
    }

    pub fn count(&self) -> usize {
        self.reps.len()
    }

    pub fn item_size() -> usize {
        OnlineRepsContainer::item_size()
    }

    fn calculate_online(&mut self) {
        let mut current = Amount::zero();
        for account in self.reps.iter() {
            current += self.rep_weights.weight(account);
        }
        self.online = current;
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
}

pub(super) static DEFAULT_ONLINE_WEIGHT_MINIMUM: Amount = Amount::nano(60_000_000);
