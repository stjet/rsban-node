use std::sync::Arc;

use rsnano_core::utils::NullLogger;
use rsnano_store_lmdb::{EnvOptions, LmdbStore, TestDbFile};
use rsnano_store_traits::NullTransactionTracker;

use crate::{
    ledger::{GenerateCache, Ledger},
    stats::{Stat, StatConfig},
    DEV_CONSTANTS,
};

use super::AccountBlockFactory;

pub(crate) struct LedgerContext {
    pub(crate) ledger: Ledger,
    db_file: TestDbFile,
}

impl LedgerContext {
    pub fn empty() -> Self {
        let db_file = TestDbFile::random();

        let store = Arc::new(
            LmdbStore::new(
                &db_file.path,
                &EnvOptions::default(),
                Arc::new(NullTransactionTracker::new()),
                Arc::new(NullLogger::new()),
                false,
            )
            .unwrap(),
        );

        let ledger = Ledger::new(
            store.clone(),
            DEV_CONSTANTS.clone(),
            Arc::new(Stat::new(StatConfig::default())),
            &GenerateCache::new(),
        )
        .unwrap();

        let mut txn = store.tx_begin_write().unwrap();
        store.initialize(
            &mut txn,
            &ledger.cache,
            &DEV_CONSTANTS.genesis.read().unwrap(),
            DEV_CONSTANTS.final_votes_canary_account,
            DEV_CONSTANTS.final_votes_canary_height,
        );

        LedgerContext { ledger, db_file }
    }

    pub fn genesis_block_factory(&self) -> AccountBlockFactory {
        AccountBlockFactory::genesis(&self.ledger)
    }

    pub fn block_factory(&self) -> AccountBlockFactory {
        AccountBlockFactory::new(&self.ledger)
    }
}
