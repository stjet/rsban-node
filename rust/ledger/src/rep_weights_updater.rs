use crate::RepWeightCache;
use rsnano_core::{Account, Amount};
use rsnano_store_lmdb::{LmdbRepWeightStore, LmdbWriteTransaction};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Updates the representative weights in the ledger and in the in-memory cache
pub struct RepWeightsUpdater {
    weight_cache: Arc<RwLock<HashMap<Account, Amount>>>,
    store: Arc<LmdbRepWeightStore>,
    min_weight: Amount,
}

impl RepWeightsUpdater {
    pub fn new(store: Arc<LmdbRepWeightStore>, min_weight: Amount) -> Self {
        RepWeightsUpdater {
            weight_cache: Arc::new(RwLock::new(HashMap::new())),
            store,
            min_weight,
        }
    }

    pub fn cache(&self) -> RepWeightCache {
        RepWeightCache::new(self.weight_cache.clone())
    }

    /// Only use this method when loading rep weights from the database table
    pub fn copy_from(&self, other: &HashMap<Account, Amount>) {
        let mut guard_this = self.weight_cache.write().unwrap();
        for (account, amount) in other {
            let prev_amount = self.get(&guard_this, account);
            self.put_cache(&mut guard_this, *account, prev_amount.wrapping_add(*amount));
        }
    }

    fn get(&self, weights: &HashMap<Account, Amount>, account: &Account) -> Amount {
        weights.get(account).cloned().unwrap_or_default()
    }

    pub fn representation_add(
        &self,
        tx: &mut LmdbWriteTransaction,
        representative: Account,
        amount: Amount,
    ) {
        let previous_weight = self.store.get(tx, representative).unwrap_or_default();
        let new_weight = previous_weight.wrapping_add(amount);
        self.put_store(tx, representative, previous_weight, new_weight);
        let mut guard = self.weight_cache.write().unwrap();
        self.put_cache(&mut guard, representative, new_weight);
    }

    fn put_cache(
        &self,
        weights: &mut HashMap<Account, Amount>,
        representative: Account,
        new_weight: Amount,
    ) {
        if new_weight < self.min_weight || new_weight.is_zero() {
            weights.remove(&representative);
        } else {
            weights.insert(representative, new_weight);
        }
    }

    fn put_store(
        &self,
        tx: &mut LmdbWriteTransaction,
        representative: Account,
        previous_weight: Amount,
        new_weight: Amount,
    ) {
        if new_weight.is_zero() {
            if !previous_weight.is_zero() {
                self.store.del(tx, representative);
            }
        } else {
            self.store.put(tx, representative, new_weight);
        }
    }

    /// Only use this method when loading rep weights from the database table!
    pub fn representation_put(&self, representative: Account, weight: Amount) {
        let mut guard = self.weight_cache.write().unwrap();
        self.put_cache(&mut guard, representative, weight);
    }

    pub fn representation_add_dual(
        &self,
        tx: &mut LmdbWriteTransaction,
        rep_1: Account,
        amount_1: Amount,
        rep_2: Account,
        amount_2: Amount,
    ) {
        if rep_1 != rep_2 {
            let previous_weight_1 = self.store.get(tx, rep_1).unwrap_or_default();
            let previous_weight_2 = self.store.get(tx, rep_2).unwrap_or_default();
            let new_weight_1 = previous_weight_1.wrapping_add(amount_1);
            let new_weight_2 = previous_weight_2.wrapping_add(amount_2);
            self.put_store(tx, rep_1, previous_weight_1, new_weight_1);
            self.put_store(tx, rep_2, previous_weight_2, new_weight_2);
            let mut guard = self.weight_cache.write().unwrap();
            self.put_cache(&mut guard, rep_1, new_weight_1);
            self.put_cache(&mut guard, rep_2, new_weight_2);
        } else {
            self.representation_add(tx, rep_1, amount_1.wrapping_add(amount_2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_store_lmdb::{ConfiguredRepWeightDatabaseBuilder, LmdbEnv};

    #[test]
    fn representation_changes() {
        let env = Arc::new(LmdbEnv::new_null());
        let store = Arc::new(LmdbRepWeightStore::new(env).unwrap());
        let account = Account::from(1);
        let rep_weights_updater = RepWeightsUpdater::new(store, Amount::zero());
        let rep_weights = rep_weights_updater.cache();
        assert_eq!(rep_weights.get_weight(&account), Amount::zero());

        rep_weights_updater.representation_put(account, Amount::from(1));
        assert_eq!(rep_weights.get_weight(&account), Amount::from(1));

        rep_weights_updater.representation_put(account, Amount::from(2));
        assert_eq!(rep_weights.get_weight(&account), Amount::from(2));
    }

    #[test]
    fn delete_rep_weight_of_zero() {
        let representative = Account::from(1);
        let weight = Amount::from(100);

        let env = Arc::new(
            LmdbEnv::new_null_with()
                .configured_database(ConfiguredRepWeightDatabaseBuilder::create(vec![(
                    representative,
                    weight,
                )]))
                .build(),
        );
        let store = Arc::new(LmdbRepWeightStore::new(Arc::clone(&env)).unwrap());
        let delete_tracker = store.track_deletions();
        let rep_weights_updater = RepWeightsUpdater::new(store, Amount::zero());
        let rep_weights = rep_weights_updater.cache();
        rep_weights_updater.representation_put(representative, weight);
        let mut tx = env.tx_begin_write();

        // set weight to 0
        rep_weights_updater.representation_add(
            &mut tx,
            representative,
            Amount::zero().wrapping_sub(weight),
        );

        assert_eq!(rep_weights.len(), 0);
        assert_eq!(delete_tracker.output(), vec![representative]);
    }

    #[test]
    fn delete_rep_weight_of_zero_dual() {
        let rep1 = Account::from(1);
        let rep2 = Account::from(2);
        let weight = Amount::from(100);

        let env = Arc::new(
            LmdbEnv::new_null_with()
                .configured_database(ConfiguredRepWeightDatabaseBuilder::create(vec![
                    (rep1, weight),
                    (rep2, weight),
                ]))
                .build(),
        );
        let store = Arc::new(LmdbRepWeightStore::new(Arc::clone(&env)).unwrap());
        let delete_tracker = store.track_deletions();
        let rep_weights_updater = RepWeightsUpdater::new(store, Amount::zero());
        let rep_weights = rep_weights_updater.cache();
        rep_weights_updater.representation_put(rep1, weight);
        rep_weights_updater.representation_put(rep2, weight);
        let mut tx = env.tx_begin_write();

        // set weight to 0
        rep_weights_updater.representation_add_dual(
            &mut tx,
            rep1,
            Amount::zero().wrapping_sub(weight),
            rep2,
            Amount::zero().wrapping_sub(weight),
        );

        assert_eq!(rep_weights.len(), 0);
        assert_eq!(delete_tracker.output(), vec![rep1, rep2]);
    }

    #[test]
    fn add_below_min_weight() {
        let env = Arc::new(LmdbEnv::new_null());
        let store = Arc::new(LmdbRepWeightStore::new(Arc::clone(&env)).unwrap());
        let put_tracker = store.track_puts();
        let mut txn = env.tx_begin_write();
        let representative = Account::from(1);
        let min_weight = Amount::from(10);
        let rep_weight = Amount::from(9);
        let rep_weights_updater = RepWeightsUpdater::new(store, min_weight);

        rep_weights_updater.representation_add(&mut txn, representative, rep_weight);

        assert_eq!(rep_weights_updater.cache().len(), 0);
        assert_eq!(put_tracker.output(), vec![(representative, rep_weight)]);
    }

    #[test]
    fn fall_below_min_weight() {
        let representative = Account::from(1);
        let weight = Amount::from(11);
        let env = Arc::new(
            LmdbEnv::new_null_with()
                .configured_database(ConfiguredRepWeightDatabaseBuilder::create(vec![(
                    representative,
                    weight,
                )]))
                .build(),
        );
        let store = Arc::new(LmdbRepWeightStore::new(Arc::clone(&env)).unwrap());
        let put_tracker = store.track_puts();
        let mut txn = env.tx_begin_write();
        let min_weight = Amount::from(10);
        let rep_weights_updater = RepWeightsUpdater::new(store, min_weight);
        let rep_weights = rep_weights_updater.cache();

        rep_weights_updater.representation_add(
            &mut txn,
            representative,
            Amount::zero().wrapping_sub(Amount::from(2)),
        );

        assert_eq!(rep_weights.len(), 0);
        assert_eq!(put_tracker.output(), vec![(representative, 9.into())]);
    }
}