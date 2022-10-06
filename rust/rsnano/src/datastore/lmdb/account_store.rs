use std::sync::{Arc, Mutex};

use crate::{
    datastore::{lmdb::MDB_NOTFOUND, parallel_traversal, AccountStore, DbIterator, NullIterator},
    utils::{Deserialize, StreamAdapter},
    Account, AccountInfo,
};
use anyhow::Result;

use super::{
    assert_success, iterator::LmdbIterator, mdb_count, mdb_dbi_open, mdb_del, mdb_get, mdb_put,
    LmdbEnv, LmdbReadTransaction, LmdbWriteTransaction, MdbVal, OwnedMdbVal, Transaction,
    MDB_SUCCESS,
};

pub struct LmdbAccountStore {
    /// Maps account v0 to account information, head, rep, open, balance, timestamp, block count and epoch
    /// nano::account -> nano::block_hash, nano::block_hash, nano::block_hash, nano::amount, uint64_t, uint64_t, nano::epoch
    db_handle: Mutex<u32>,
    env: Arc<LmdbEnv>,
}

impl LmdbAccountStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            db_handle: Mutex::new(0),
            env,
        }
    }

    pub fn db_handle(&self) -> u32 {
        *self.db_handle.lock().unwrap()
    }

    pub fn open_db(&self, transaction: &Transaction, flags: u32) -> Result<()> {
        let mut handle = 0;
        let status =
            unsafe { mdb_dbi_open(transaction.handle(), Some("accounts"), flags, &mut handle) };
        *self.db_handle.lock().unwrap() = handle;
        if status != MDB_SUCCESS {
            bail!("could not open accounts database");
        }
        Ok(())
    }
}

impl AccountStore<LmdbReadTransaction, LmdbWriteTransaction> for LmdbAccountStore {
    fn put(&self, transaction: &LmdbWriteTransaction, account: &Account, info: &AccountInfo) {
        let mut account_val = OwnedMdbVal::from(account);
        let mut info_val = OwnedMdbVal::from(info);

        let status = unsafe {
            mdb_put(
                transaction.handle,
                self.db_handle(),
                account_val.as_mdb_val(),
                info_val.as_mdb_val(),
                0,
            )
        };
        assert_success(status);
    }

    fn get(&self, transaction: &Transaction, account: &Account) -> Option<AccountInfo> {
        let mut account_val = OwnedMdbVal::from(account);
        let mut value = MdbVal::new();
        let status1 = unsafe {
            mdb_get(
                transaction.handle(),
                self.db_handle(),
                account_val.as_mdb_val(),
                &mut value,
            )
        };
        assert!(status1 == MDB_SUCCESS || status1 == MDB_NOTFOUND);
        if status1 == MDB_SUCCESS {
            let mut stream = StreamAdapter::new(unsafe {
                std::slice::from_raw_parts(value.mv_data as *const u8, value.mv_size)
            });
            AccountInfo::deserialize(&mut stream).ok()
        } else {
            None
        }
    }

    fn del(&self, transaction: &LmdbWriteTransaction, account: &Account) {
        let mut key_val = OwnedMdbVal::from(account);
        let status = unsafe {
            mdb_del(
                transaction.handle,
                self.db_handle(),
                key_val.as_mdb_val(),
                None,
            )
        };
        assert_success(status);
    }

    fn begin_account(
        &self,
        transaction: &Transaction,
        account: &Account,
    ) -> Box<dyn DbIterator<Account, AccountInfo>> {
        Box::new(LmdbIterator::new(
            transaction,
            self.db_handle(),
            Some(account),
            true,
        ))
    }

    fn begin(&self, transaction: &Transaction) -> Box<dyn DbIterator<Account, AccountInfo>> {
        Box::new(LmdbIterator::new(transaction, self.db_handle(), None, true))
    }

    fn rbegin(&self, transaction: &Transaction) -> Box<dyn DbIterator<Account, AccountInfo>> {
        Box::new(LmdbIterator::new(
            transaction,
            self.db_handle(),
            None,
            false,
        ))
    }

    fn end(&self) -> Box<dyn DbIterator<Account, AccountInfo>> {
        Box::new(NullIterator::new())
    }

    fn for_each_par(
        &self,
        action: &(dyn Fn(
            &LmdbReadTransaction,
            &mut dyn DbIterator<Account, AccountInfo>,
            &mut dyn DbIterator<Account, AccountInfo>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let mut transaction = self.env.tx_begin_read();
            let mut begin_it = self.begin_account(&transaction.as_txn(), &start.into());
            let mut end_it = if !is_last {
                self.begin_account(&transaction.as_txn(), &end.into())
            } else {
                self.end()
            };
            action(&mut transaction, begin_it.as_mut(), end_it.as_mut());
        });
    }

    fn count(&self, txn: &Transaction) -> usize {
        unsafe { mdb_count(txn.handle(), self.db_handle()) }
    }
}
