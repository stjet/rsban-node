use crate::{ledger_constants::LEDGER_CONSTANTS_STUB, Ledger};
use rsnano_core::{Account, ConfirmationHeightInfo};
use rsnano_store_lmdb::{EnvironmentWrapper, LmdbStore, LmdbWriteTransaction, TestDbFile};
use std::sync::Arc;

#[cfg(test)]
use crate::ledger_tests::helpers::AccountBlockFactory;

pub struct LedgerContext {
    pub ledger: Arc<Ledger>,
    _db_file: TestDbFile,
}

impl LedgerContext {
    pub fn empty() -> Self {
        let db_file = TestDbFile::random();
        let store = Arc::new(
            LmdbStore::<EnvironmentWrapper>::open(&db_file.path)
                .build()
                .unwrap(),
        );
        let ledger = Arc::new(Ledger::new(store.clone(), LEDGER_CONSTANTS_STUB.clone()).unwrap());

        LedgerContext {
            ledger,
            _db_file: db_file,
        }
    }

    #[cfg(test)]
    pub(crate) fn genesis_block_factory(&self) -> AccountBlockFactory {
        AccountBlockFactory::genesis(&self.ledger)
    }

    #[cfg(test)]
    pub(crate) fn block_factory(&self) -> AccountBlockFactory {
        AccountBlockFactory::new(&self.ledger)
    }

    pub fn inc_confirmation_height(&self, txn: &mut LmdbWriteTransaction, account: &Account) {
        let mut height = self
            .ledger
            .store
            .confirmation_height
            .get(txn, account)
            .unwrap_or_else(|| ConfirmationHeightInfo {
                height: 0,
                frontier: self.ledger.account_info(txn, account).unwrap().head,
            });
        height.height = height.height + 1;
        self.ledger
            .store
            .confirmation_height
            .put(txn, account, &height);
    }
}
