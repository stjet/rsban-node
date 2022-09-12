use std::sync::Arc;

use crate::{
    datastore::{
        lmdb::{MDB_NOTFOUND, MDB_SUCCESS},
        ConfirmationHeightStore, DbIterator, Transaction, WriteTransaction,
    },
    utils::Deserialize,
    Account, ConfirmationHeightInfo,
};

use super::{
    assert_success, exists, get_raw_lmdb_txn, mdb_count, mdb_del, mdb_drop, mdb_get, mdb_put,
    LmdbEnv, LmdbIterator, MdbVal, OwnedMdbVal,
};

pub struct LmdbConfirmationHeightStore {
    env: Arc<LmdbEnv>,
    pub table_handle: u32,
}

impl LmdbConfirmationHeightStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            table_handle: 0,
        }
    }
}

impl ConfirmationHeightStore for LmdbConfirmationHeightStore {
    fn put(
        &self,
        txn: &dyn WriteTransaction,
        account: &crate::Account,
        info: &ConfirmationHeightInfo,
    ) {
        let mut key = MdbVal::from_slice(account.as_bytes());
        let mut value = OwnedMdbVal::from(info);
        let status = unsafe {
            mdb_put(
                get_raw_lmdb_txn(txn.as_transaction()),
                self.table_handle,
                &mut key,
                value.as_mdb_val(),
                0,
            )
        };
        assert_success(status);
    }

    fn get(
        &self,
        txn: &dyn crate::datastore::Transaction,
        account: &crate::Account,
    ) -> Option<ConfirmationHeightInfo> {
        let mut key = MdbVal::from(account);
        let mut data = MdbVal::new();
        let status = unsafe {
            mdb_get(
                get_raw_lmdb_txn(txn),
                self.table_handle,
                &mut key,
                &mut data,
            )
        };
        assert!(status == MDB_SUCCESS || status == MDB_NOTFOUND);

        if status == MDB_SUCCESS {
            let mut stream = data.as_stream();
            ConfirmationHeightInfo::deserialize(&mut stream).ok()
        } else {
            None
        }
    }

    fn exists(&self, txn: &dyn Transaction, account: &Account) -> bool {
        exists(txn, self.table_handle, &mut MdbVal::from(account))
    }

    fn del(&self, txn: &dyn Transaction, account: &Account) {
        let status = unsafe {
            mdb_del(
                get_raw_lmdb_txn(txn),
                self.table_handle,
                &mut MdbVal::from(account),
                None,
            )
        };
        assert_success(status);
    }

    fn count(&self, txn: &dyn Transaction) -> usize {
        unsafe { mdb_count(get_raw_lmdb_txn(txn), self.table_handle) }
    }

    fn clear(&self, txn: &dyn WriteTransaction) {
        unsafe { mdb_drop(get_raw_lmdb_txn(txn.as_transaction()), self.table_handle, 0) };
    }

    fn begin(&self, txn: &dyn Transaction) -> Box<dyn DbIterator<Account, ConfirmationHeightInfo>> {
        Box::new(LmdbIterator::new(txn, self.table_handle, None, true))
    }

    fn begin_at_account(
        &self,
        txn: &dyn Transaction,
        account: &Account,
    ) -> Box<dyn DbIterator<Account, ConfirmationHeightInfo>> {
        Box::new(LmdbIterator::new(
            txn,
            self.table_handle,
            Some(account),
            true,
        ))
    }
}
