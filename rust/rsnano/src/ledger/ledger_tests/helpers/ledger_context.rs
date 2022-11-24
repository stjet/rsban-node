use std::{sync::Arc, time::Duration};

use crate::{
    config::TxnTrackingConfig,
    ledger::{
        datastore::lmdb::{EnvOptions, LmdbStore, TestDbFile},
        GenerateCache, Ledger,
    },
    stats::{Stat, StatConfig},
    utils::NullLogger,
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
                TxnTrackingConfig::default(),
                Duration::from_millis(5000),
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
        store.initialize(&mut txn, &ledger.cache, &DEV_CONSTANTS);

        LedgerContext { ledger, db_file }
    }

    pub fn genesis_block_factory(&self) -> AccountBlockFactory {
        AccountBlockFactory::genesis(&self.ledger)
    }

    pub fn block_factory(&self) -> AccountBlockFactory {
        AccountBlockFactory::new(&self.ledger)
    }
}
