use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use rsnano_core::Account;
use rsnano_ledger::Ledger;
use rsnano_store_lmdb::LmdbWalletStore;

pub struct Wallet {
    pub representatives: Mutex<HashSet<Account>>,
    store: Arc<LmdbWalletStore>,
    ledger: Arc<Ledger>,
}

impl Wallet {
    pub fn new(store: Arc<LmdbWalletStore>, ledger: Arc<Ledger>) -> Self {
        Self {
            representatives: Mutex::new(HashSet::new()),
            store,
            ledger,
        }
    }
}
