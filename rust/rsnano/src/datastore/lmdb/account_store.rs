use std::sync::Arc;

use crate::{
    datastore::{
        lmdb::MDB_NOTFOUND, AccountStore, DbIterator, ReadTransaction, Transaction,
        WriteTransaction,
    },
    utils::{Deserialize, StreamAdapter},
    Account, AccountInfo,
};
use anyhow::Result;

use super::{
    assert_success, get_raw_lmdb_txn, iterator::LmdbIterator, mdb_dbi_open, mdb_del, mdb_get,
    mdb_put, LmdbEnv, MdbVal, OwnedMdbVal, MDB_SUCCESS,
};

pub struct LmdbAccountStore {
    /// Maps account v0 to account information, head, rep, open, balance, timestamp, block count and epoch
    /// nano::account -> nano::block_hash, nano::block_hash, nano::block_hash, nano::amount, uint64_t, uint64_t, nano::epoch
    pub accounts_handle: u32,

    env: Arc<LmdbEnv>,
}

impl LmdbAccountStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            accounts_handle: 0,
            env,
        }
    }

    pub fn open_databases(&mut self, transaction: &dyn Transaction, flags: u32) -> Result<()> {
        let txn = get_raw_lmdb_txn(transaction);
        let status = unsafe { mdb_dbi_open(txn, "accounts", flags, &mut self.accounts_handle) };
        if status != MDB_SUCCESS {
            bail!("could not open accounts database");
        }
        Ok(())
    }
}

impl AccountStore for LmdbAccountStore {
    fn put(&self, transaction: &dyn WriteTransaction, account: &Account, info: &AccountInfo) {
        let mut account_val = OwnedMdbVal::from(account);
        let mut info_val = OwnedMdbVal::from(info);

        let status = unsafe {
            mdb_put(
                get_raw_lmdb_txn(transaction.as_transaction()),
                self.accounts_handle,
                account_val.as_mdb_val(),
                info_val.as_mdb_val(),
                0,
            )
        };
        assert_success(status);
    }

    fn get(&self, transaction: &dyn Transaction, account: &Account) -> Option<AccountInfo> {
        let mut account_val = OwnedMdbVal::from(account);
        let mut value = MdbVal::new();
        let status1 = unsafe {
            mdb_get(
                get_raw_lmdb_txn(transaction),
                self.accounts_handle,
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

    fn del(&self, transaction: &dyn WriteTransaction, account: &Account) {
        let mut key_val = OwnedMdbVal::from(account);
        let status = unsafe {
            mdb_del(
                get_raw_lmdb_txn(transaction.as_transaction()),
                self.accounts_handle,
                key_val.as_mdb_val(),
                None,
            )
        };
        assert_success(status);
    }

    fn begin_account(
        &self,
        transaction: &dyn Transaction,
        account: &Account,
    ) -> Box<dyn DbIterator<Account, AccountInfo>> {
        Box::new(LmdbIterator::new(
            transaction,
            self.accounts_handle,
            Some(account),
            true,
        ))
    }

    fn begin(&self, transaction: &dyn Transaction) -> Box<dyn DbIterator<Account, AccountInfo>> {
        Box::new(LmdbIterator::new(
            transaction,
            self.accounts_handle,
            None,
            true,
        ))
    }

    fn rbegin(&self, transaction: &dyn Transaction) -> Box<dyn DbIterator<Account, AccountInfo>> {
        Box::new(LmdbIterator::new(
            transaction,
            self.accounts_handle,
            None,
            false,
        ))
    }

    fn for_each_par(
        &self,
        _action: &dyn Fn(
            &dyn ReadTransaction,
            &mut dyn DbIterator<Account, AccountInfo>,
            &mut dyn DbIterator<Account, AccountInfo>,
        ),
    ) {
        todo!()
        // parallel_traversal(&|start, end, is_last|{
        //     let transaction (this->store.tx_begin_read ());
        //     action_a (*transaction, this->begin (*transaction, start), !is_last ? this->begin (*transaction, end) : this->end ());
        // });
    }
}
