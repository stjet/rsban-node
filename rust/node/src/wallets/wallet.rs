use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use rsnano_core::{Account, BlockHash, KeyPair, PendingKey};
use rsnano_ledger::Ledger;
use rsnano_store_lmdb::{EnvironmentWrapper, LmdbWalletStore, Transaction};

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

    pub fn deterministic_check(
        &self,
        txn: &dyn Transaction<
            Database = <EnvironmentWrapper as rsnano_store_lmdb::Environment>::Database,
            RoCursor = <EnvironmentWrapper as rsnano_store_lmdb::Environment>::RoCursor,
        >,
        index: u32,
    ) -> u32 {
        let mut result = index;
        let block_txn = self.ledger.read_txn();
        let mut i = index + 1;
        let mut n = index + 64;
        while i < n {
            let prv = self.store.deterministic_key(txn, i);
            let pair = KeyPair::from_priv_key_bytes(prv.as_bytes()).unwrap();
            // Check if account received at least 1 block
            let latest = self.ledger.latest(&block_txn, &pair.public_key());
            match latest {
                Some(_) => {
                    result = i;
                    // i + 64 - Check additional 64 accounts
                    // i/64 - Check additional accounts for large wallets. I.e. 64000/64 = 1000 accounts to check
                    n = i + 64 + (i / 64);
                }
                None => {
                    // Check if there are pending blocks for account
                    let pending_it = self.ledger.store.pending.begin_at_key(
                        &block_txn,
                        &PendingKey::new(pair.public_key(), BlockHash::from(0)),
                    );
                    if let Some((key, _)) = pending_it.current() {
                        if key.account == pair.public_key() {
                            result = i;
                            n = i + 64 + (i / 64);
                        }
                    }
                }
            }

            i += 1;
        }
        result
    }

    pub fn live(&self) -> bool {
        self.store.is_open()
    }
}
