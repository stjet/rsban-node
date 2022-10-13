use lmdb::{Database, DatabaseFlags, WriteFlags};
use std::sync::Arc;

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
    database: Database,
}

impl LmdbConfirmationHeightStore {
    pub fn new(env: Arc<LmdbEnv>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("confirmation_height"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> Database {
        self.database
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
        txn.rw_txn_mut()
            .put(
                self.database,
                account.as_bytes(),
                &info.to_bytes(),
                WriteFlags::empty(),
            )
            .unwrap();
    }

    fn get(&self, txn: &LmdbTransaction, account: &Account) -> Option<ConfirmationHeightInfo> {
        match txn.get(self.database, account.as_bytes()) {
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
        exists(txn, self.database, account.as_bytes())
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, account: &Account) {
        txn.rw_txn_mut()
            .del(self.database, account.as_bytes(), None)
            .unwrap();
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.database)
    }

    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut().clear_db(self.database).unwrap()
    }

    fn begin(&self, txn: &LmdbTransaction) -> ConfirmationHeightIterator<LmdbIteratorImpl> {
        DbIterator2::new(LmdbIteratorImpl::new(txn, self.database, None, true))
    }

    fn begin_at_account(
        &self,
        txn: &LmdbTransaction,
        account: &Account,
    ) -> ConfirmationHeightIterator<LmdbIteratorImpl> {
        DbIterator2::new(LmdbIteratorImpl::new(
            txn,
            self.database,
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
