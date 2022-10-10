use std::sync::{Arc, Mutex};

use lmdb::{Database, DatabaseFlags, WriteFlags};

use crate::{
    datastore::{
        confirmation_height_store::ConfirmationHeightIterator, parallel_traversal,
        ConfirmationHeightStore, DbIterator2,
    },
    utils::{Deserialize, StreamAdapter},
    Account, ConfirmationHeightInfo,
};

use super::{
    exists, LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

pub struct LmdbConfirmationHeightStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<Option<Database>>,
}

impl LmdbConfirmationHeightStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            db_handle: Mutex::new(None),
        }
    }

    pub fn db_handle(&self) -> Database {
        self.db_handle.lock().unwrap().unwrap()
    }

    pub fn create_db(&self) -> anyhow::Result<()> {
        let db = self
            .env
            .environment
            .create_db(Some("confirmation_height"), DatabaseFlags::empty())?;
        *self.db_handle.lock().unwrap() = Some(db);
        Ok(())
    }
}

impl<'a>
    ConfirmationHeightStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbConfirmationHeightStore
{
    fn put(
        &self,
        txn: &mut LmdbWriteTransaction,
        account: &Account,
        info: &ConfirmationHeightInfo,
    ) {
        txn.rw_txn()
            .put(
                self.db_handle(),
                account.as_bytes(),
                &info.to_bytes(),
                WriteFlags::empty(),
            )
            .unwrap();
    }

    fn get(&self, txn: &LmdbTransaction, account: &Account) -> Option<ConfirmationHeightInfo> {
        match txn.get(self.db_handle(), account.as_bytes()) {
            Err(lmdb::Error::NotFound) => None,
            Ok(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                ConfirmationHeightInfo::deserialize(&mut stream).ok()
            }
            Err(e) => {
                panic!("Could not load confirmation height info: {:?}", e);
            }
        }
    }

    fn exists(&self, txn: &LmdbTransaction, account: &Account) -> bool {
        exists(txn, self.db_handle(), account.as_bytes())
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, account: &Account) {
        txn.rw_txn()
            .del(self.db_handle(), account.as_bytes(), None)
            .unwrap();
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.db_handle())
    }

    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn().clear_db(self.db_handle()).unwrap()
    }

    fn begin(&self, txn: &LmdbTransaction) -> ConfirmationHeightIterator<LmdbIteratorImpl> {
        DbIterator2::new(LmdbIteratorImpl::new(txn, self.db_handle(), None, true))
    }

    fn begin_at_account(
        &self,
        txn: &LmdbTransaction,
        account: &Account,
    ) -> ConfirmationHeightIterator<LmdbIteratorImpl> {
        DbIterator2::new(LmdbIteratorImpl::new(
            txn,
            self.db_handle(),
            Some(account.as_bytes()),
            true,
        ))
    }

    fn end(&self) -> ConfirmationHeightIterator<LmdbIteratorImpl> {
        DbIterator2::new(LmdbIteratorImpl::null())
    }

    fn for_each_par(
        &'a self,
        action: &(dyn Fn(
            LmdbReadTransaction<'a>,
            ConfirmationHeightIterator<LmdbIteratorImpl>,
            ConfirmationHeightIterator<LmdbIteratorImpl>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read().unwrap();
            let begin_it = self.begin_at_account(&transaction.as_txn(), &start.into());
            let end_it = if !is_last {
                self.begin_at_account(&transaction.as_txn(), &end.into())
            } else {
                self.end()
            };
            action(transaction, begin_it, end_it);
        });
    }
}
