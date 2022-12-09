use std::sync::Arc;

use rsnano_core::{utils::NullLogger, Account, ConfirmationHeightInfo};
use rsnano_ledger::GenerateCache;
use rsnano_store_lmdb::{EnvOptions, LmdbStore, TestDbFile};
use rsnano_store_traits::{NullTransactionTracker, WriteTransaction};

use crate::{
    ledger::Ledger,
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

        LedgerContext { ledger, db_file }
    }

    pub fn genesis_block_factory(&self) -> AccountBlockFactory {
        AccountBlockFactory::genesis(&self.ledger)
    }

    pub fn block_factory(&self) -> AccountBlockFactory {
        AccountBlockFactory::new(&self.ledger)
    }

    pub fn inc_confirmation_height(&self, txn: &mut dyn WriteTransaction, account: &Account) {
        let mut height = self
            .ledger
            .store
            .confirmation_height()
            .get(txn.txn(), account)
            .unwrap_or_else(|| ConfirmationHeightInfo {
                height: 0,
                frontier: self
                    .ledger
                    .store
                    .account()
                    .get(txn.txn(), account)
                    .unwrap()
                    .head,
            });
        height.height = height.height + 1;
        self.ledger
            .store
            .confirmation_height()
            .put(txn, account, &height);
    }
}
