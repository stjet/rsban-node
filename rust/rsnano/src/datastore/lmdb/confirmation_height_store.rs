use std::sync::Arc;

use crate::{
    datastore::{ConfirmationHeightStore, WriteTransaction},
    ConfirmationHeightInfo,
};

use super::{assert_success, mdb_put, LmdbEnv, LmdbWriteTransaction, MdbVal, OwnedMdbVal};

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
}
