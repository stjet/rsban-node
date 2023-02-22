use crate::OnlineRepsContainer;
use primitive_types::U256;
use rsnano_core::{Account, Amount};
use rsnano_ledger::Ledger;
use rsnano_store_traits::WriteTransaction;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{cmp::max, sync::Arc};

#[cfg(test)]
use mock_instant::Instant;
#[cfg(not(test))]
use std::time::Instant;

pub const ONLINE_WEIGHT_QUORUM: u8 = 67;
static DEFAULT_ONLINE_WEIGHT_MINIMUM: Amount = Amount::nano(60_000_000);

pub struct OnlineReps {
    ledger: Arc<Ledger>,
    reps: OnlineRepsContainer,
    trended: Amount,
    online: Amount,
    minimum: Amount,
    weight_period: Duration,
    online_weight_minimum: Amount,
}

impl OnlineReps {
    pub fn new(ledger: Arc<Ledger>) -> Self {
        Self {
            ledger,
            reps: OnlineRepsContainer::new(),
            trended: Amount::zero(),
            online: Amount::zero(),
            minimum: Amount::zero(),
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
        if self.ledger.weight(&rep_account) > Amount::zero() {
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

    /** Returns the quorum required for confirmation*/
    pub fn delta(&self) -> Amount {
        // Using a larger container to ensure maximum precision
        let weight = max(max(self.online, self.trended), self.online_weight_minimum);

        let delta =
            U256::from(weight.number()) * U256::from(ONLINE_WEIGHT_QUORUM) / U256::from(100);
        return Amount::raw(delta.as_u128());
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
            current += self.ledger.weight(account);
        }
        self.online = current;
    }
}

pub struct OnlineWeightSampler {
    ledger: Arc<Ledger>,
    online_weight_minimum: Amount,
    max_samples: u64,
}

impl OnlineWeightSampler {
    pub fn new(ledger: Arc<Ledger>) -> Self {
        Self {
            ledger,
            online_weight_minimum: DEFAULT_ONLINE_WEIGHT_MINIMUM,
            max_samples: 4032,
        }
    }

    pub fn set_online_weight_minimum(&mut self, minimum: Amount) {
        self.online_weight_minimum = minimum;
    }

    pub fn set_max_samples(&mut self, max_samples: u64) {
        self.max_samples = max_samples;
    }

    pub fn calculate_trend(&mut self) -> Amount {
        self.medium_weight(self.load_samples())
    }

    fn load_samples(&self) -> Vec<Amount> {
        let txn = self.ledger.read_txn();
        let mut items = Vec::with_capacity(self.max_samples as usize + 1);
        items.push(self.online_weight_minimum);
        let mut it = self.ledger.store.online_weight().begin(txn.txn());
        while !it.is_end() {
            items.push(*it.current().unwrap().1);
            it.next();
        }
        items
    }

    fn medium_weight(&self, mut items: Vec<Amount>) -> Amount {
        let median_idx = items.len() / 2;
        items.sort();
        items[median_idx]
    }

    /** Called periodically to sample online weight */
    pub fn sample(&self, current_online_weight: Amount) {
        let mut txn = self.ledger.rw_txn();
        self.delete_old_samples(txn.as_mut());
        self.insert_new_sample(txn.as_mut(), current_online_weight);
    }

    fn delete_old_samples(&self, txn: &mut dyn WriteTransaction) {
        let weight_store = self.ledger.store.online_weight();

        while weight_store.count(txn.txn()) >= self.max_samples {
            let (&oldest, _) = weight_store.begin(txn.txn()).current().unwrap();
            weight_store.del(txn, oldest);
        }
    }

    fn insert_new_sample(&self, txn: &mut dyn WriteTransaction, current_online_weight: Amount) {
        self.ledger.store.online_weight().put(
            txn,
            nano_seconds_since_epoch(),
            &current_online_weight,
        );
    }
}

fn nano_seconds_since_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos() as u64
}
