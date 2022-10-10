use std::sync::{Arc, Mutex};

use crate::{
    datastore::{
        lmdb::{MDB_NOTFOUND, MDB_SUCCESS},
        parallel_traversal, ConfirmationHeightStore, DbIterator, NullIterator,
    },
    utils::Deserialize,
    Account, ConfirmationHeightInfo,
};

use super::{
    assert_success, ensure_success, exists, mdb_count, mdb_dbi_open, mdb_del, mdb_drop, mdb_get,
    mdb_put, LmdbEnv, LmdbIterator, LmdbReadTransaction, LmdbWriteTransaction, MdbVal, OwnedMdbVal,
    Transaction,
};

pub struct LmdbConfirmationHeightStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<u32>,
}

impl LmdbConfirmationHeightStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            db_handle: Mutex::new(0),
        }
    }

    pub fn db_handle(&self) -> u32 {
        *self.db_handle.lock().unwrap()
    }

    pub fn open_db(&self, txn: &Transaction, flags: u32) -> anyhow::Result<()> {
        let mut handle = 0;
        let status = unsafe {
            mdb_dbi_open(
                txn.handle(),
                Some("confirmation_height"),
                flags,
                &mut handle,
            )
        };
        *self.db_handle.lock().unwrap() = handle;
        ensure_success(status)
    }
}

impl ConfirmationHeightStore<LmdbReadTransaction, LmdbWriteTransaction>
    for LmdbConfirmationHeightStore
{
    fn put(
        &self,
        txn: &LmdbWriteTransaction,
        account: &crate::Account,
        info: &ConfirmationHeightInfo,
    ) {
        let mut key = MdbVal::from_slice(account.as_bytes());
        let mut value = OwnedMdbVal::from(info);
        let status = unsafe {
            mdb_put(
                txn.handle,
                self.db_handle(),
                &mut key,
                value.as_mdb_val(),
                0,
            )
        };
        assert_success(status);
    }

    fn get(&self, txn: &Transaction, account: &crate::Account) -> Option<ConfirmationHeightInfo> {
        let mut key = MdbVal::from(account);
        let mut data = MdbVal::new();
        let status = unsafe { mdb_get(txn.handle(), self.db_handle(), &mut key, &mut data) };
        assert!(status == MDB_SUCCESS || status == MDB_NOTFOUND);

        if status == MDB_SUCCESS {
            let mut stream = data.as_stream();
            ConfirmationHeightInfo::deserialize(&mut stream).ok()
        } else {
            None
        }
    }

    fn exists(&self, txn: &Transaction, account: &Account) -> bool {
        exists(txn, self.db_handle(), &mut MdbVal::from(account))
    }

    fn del(&self, txn: &Transaction, account: &Account) {
        let status = unsafe {
            mdb_del(
                txn.handle(),
                self.db_handle(),
                &mut MdbVal::from(account),
                None,
            )
        };
        assert_success(status);
    }

    fn count(&self, txn: &Transaction) -> usize {
        unsafe { mdb_count(txn.handle(), self.db_handle()) }
    }

    fn clear(&self, txn: &LmdbWriteTransaction) {
        unsafe { mdb_drop(txn.handle, self.db_handle(), 0) };
    }

    fn begin(&self, txn: &Transaction) -> Box<dyn DbIterator<Account, ConfirmationHeightInfo>> {
        Box::new(LmdbIterator::new(txn, self.db_handle(), None, true))
    }

    fn begin_at_account(
        &self,
        txn: &Transaction,
        account: &Account,
    ) -> Box<dyn DbIterator<Account, ConfirmationHeightInfo>> {
        Box::new(LmdbIterator::new(
            txn,
            self.db_handle(),
            Some(account),
            true,
        ))
    }

    fn end(&self) -> Box<dyn DbIterator<Account, ConfirmationHeightInfo>> {
        Box::new(NullIterator::new())
    }

    fn for_each_par(
        &self,
        action: &(dyn Fn(
            LmdbReadTransaction,
            &mut dyn DbIterator<Account, ConfirmationHeightInfo>,
            &mut dyn DbIterator<Account, ConfirmationHeightInfo>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read();
            let mut begin_it = self.begin_at_account(&transaction.as_txn(), &start.into());
            let mut end_it = if !is_last {
                self.begin_at_account(&transaction.as_txn(), &end.into())
            } else {
                self.end()
            };
            action(transaction, begin_it.as_mut(), end_it.as_mut());
        });
    }
}
