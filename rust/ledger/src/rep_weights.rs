use rsnano_core::{Account, Amount};
use rsnano_store_lmdb::{
    Environment, EnvironmentWrapper, LmdbRepWeightStore, LmdbWriteTransaction,
};
use std::collections::HashMap;
use std::mem::size_of;
use std::sync::{Arc, Mutex, MutexGuard};

pub struct RepWeights<T: Environment + 'static = EnvironmentWrapper> {
    rep_amounts: Mutex<HashMap<Account, Amount>>,
    store: Arc<LmdbRepWeightStore<T>>,
}

impl<T: Environment + 'static> RepWeights<T> {
    pub fn new(store: Arc<LmdbRepWeightStore<T>>) -> Self {
        RepWeights {
            rep_amounts: Mutex::new(HashMap::new()),
            store,
        }
    }

    fn get(&self, guard: &MutexGuard<HashMap<Account, Amount>>, account: &Account) -> Amount {
        guard.get(account).cloned().unwrap_or_default()
    }

    /// Only use this method when loading rep weights from the database table!
    fn put(
        &self,
        guard: &mut MutexGuard<HashMap<Account, Amount>>,
        account: Account,
        representation: Amount,
    ) {
        guard.insert(account, representation);
    }

    pub fn get_rep_amounts(&self) -> HashMap<Account, Amount> {
        self.rep_amounts.lock().unwrap().clone()
    }

    pub fn copy_from(&self, other: &RepWeights<T>) {
        let mut guard_this = self.rep_amounts.lock().unwrap();
        let guard_other = other.rep_amounts.lock().unwrap();
        for (account, amount) in guard_other.iter() {
            let prev_amount = self.get(&guard_this, account);
            self.put(&mut guard_this, *account, prev_amount.wrapping_add(*amount));
        }
    }

    pub fn representation_add(
        &self,
        tx: &mut LmdbWriteTransaction<T>,
        representative: Account,
        amount: Amount,
    ) {
        let weight = self.store.get(tx, representative).unwrap_or_default();
        let weight = weight.wrapping_add(amount);
        self.store.put(tx, representative, weight);
        let mut guard = self.rep_amounts.lock().unwrap();
        self.put(&mut guard, representative, weight);
    }

    pub fn representation_put(&self, representative: Account, weight: Amount) {
        let mut guard = self.rep_amounts.lock().unwrap();
        self.put(&mut guard, representative, weight);
    }

    pub fn representation_get(&self, account: &Account) -> Amount {
        let guard = self.rep_amounts.lock().unwrap();
        self.get(&guard, account)
    }

    pub fn representation_add_dual(
        &self,
        tx: &mut LmdbWriteTransaction<T>,
        rep_1: Account,
        amount_1: Amount,
        rep_2: Account,
        amount_2: Amount,
    ) {
        if rep_1 != rep_2 {
            let mut rep_1_weight = self.store.get(tx, rep_1).unwrap_or_default();
            let mut rep_2_weight = self.store.get(tx, rep_2).unwrap_or_default();
            rep_1_weight = rep_1_weight.wrapping_add(amount_1);
            rep_2_weight = rep_2_weight.wrapping_add(amount_2);
            self.store.put(tx, rep_1, rep_1_weight);
            self.store.put(tx, rep_2, rep_2_weight);
            let mut guard = self.rep_amounts.lock().unwrap();
            self.put(&mut guard, rep_1, rep_1_weight);
            self.put(&mut guard, rep_2, rep_2_weight);
        } else {
            self.representation_add(tx, rep_1, amount_1.wrapping_add(amount_2));
        }
    }

    pub fn item_size() -> usize {
        size_of::<(Account, Amount)>()
    }

    pub fn count(&self) -> usize {
        self.rep_amounts.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use crate::LedgerContext;

    use super::*;

    #[test]
    fn representation_changes() {
        let ctx = LedgerContext::empty();
        let account = Account::from(1);
        let rep_weights = RepWeights::new(Arc::clone(&ctx.ledger.store.rep_weight));
        assert_eq!(rep_weights.representation_get(&account), Amount::zero());

        rep_weights.representation_put(account, Amount::from(1));
        assert_eq!(rep_weights.representation_get(&account), Amount::from(1));

        rep_weights.representation_put(account, Amount::from(2));
        assert_eq!(rep_weights.representation_get(&account), Amount::from(2));
    }
}
