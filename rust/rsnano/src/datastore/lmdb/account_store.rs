use crate::{
    datastore::{lmdb::MDB_NOTFOUND, Transaction, WriteTransaction},
    utils::{MemoryStream, StreamAdapter},
    Account, AccountInfo,
};
use anyhow::Result;

use super::{
    assert_success, mdb_dbi_open, mdb_del, mdb_get, mdb_put, LmdbReadTransaction,
    LmdbWriteTransaction, MdbTxn, MdbVal, OwnedMdbVal, MDB_SUCCESS,
};

pub struct AccountStore {
    /// Maps account v0 to account information, head, rep, open, balance, timestamp, block count and epoch
    /// nano::account -> nano::block_hash, nano::block_hash, nano::block_hash, nano::amount, uint64_t, uint64_t, nano::epoch
    pub accounts_handle: u32,
}

impl AccountStore {
    pub fn new() -> Self {
        Self { accounts_handle: 0 }
    }

    pub fn open_databases(&mut self, transaction: &dyn Transaction, flags: u32) -> Result<()> {
        let txn = get_raw_lmdb_txn(transaction);
        let status = unsafe { mdb_dbi_open(txn, "accounts", flags, &mut self.accounts_handle) };
        if status != MDB_SUCCESS {
            bail!("could not open accounts database");
        }
        Ok(())
    }

    pub fn put(&self, transaction: &dyn WriteTransaction, account: &Account, info: &AccountInfo) {
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

    pub fn get(&self, transaction: &dyn Transaction, account: &Account) -> Option<AccountInfo> {
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

    pub fn del(&self, transaction: &dyn WriteTransaction, account: &Account) {
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
}

impl Default for AccountStore {
    fn default() -> Self {
        Self::new()
    }
}

fn get_raw_lmdb_txn(txn: &dyn Transaction) -> *mut MdbTxn {
    let any = txn.as_any();
    if let Some(t) = any.downcast_ref::<LmdbReadTransaction>() {
        t.handle
    } else if let Some(t) = any.downcast_ref::<LmdbWriteTransaction>() {
        t.handle
    } else {
        panic!("not an LMDB transaction");
    }
}

impl From<&Account> for OwnedMdbVal {
    fn from(value: &Account) -> Self {
        let mut stream = MemoryStream::new();
        value.serialize(&mut stream).unwrap();
        OwnedMdbVal::new(stream.to_vec())
    }
}

impl From<&AccountInfo> for OwnedMdbVal {
    fn from(value: &AccountInfo) -> Self {
        let mut stream = MemoryStream::new();
        value.serialize(&mut stream).unwrap();
        OwnedMdbVal::new(stream.to_vec())
    }
}
