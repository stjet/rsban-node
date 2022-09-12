use std::sync::Arc;

use crate::{
    datastore::{
        lmdb::{MDB_NOTFOUND, MDB_SUCCESS},
        ConfirmationHeightStore, Transaction, WriteTransaction,
    },
    Account, ConfirmationHeightInfo,
};

use super::{
    assert_success, exists, get_raw_lmdb_txn, mdb_get, mdb_put, LmdbEnv, LmdbWriteTransaction,
    MdbVal, OwnedMdbVal,
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
        let txn = txn.as_any().downcast_ref::<LmdbWriteTransaction>().unwrap();
        let status = unsafe {
            mdb_put(
                txn.handle,
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
}
