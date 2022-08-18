use crate::{
    datastore::{Transaction, WriteTransaction},
    utils::MemoryStream,
    Account, AccountInfo,
};
use anyhow::Result;

use super::{
    ensure_success, mdb_dbi_open, mdb_put, LmdbReadTransaction, LmdbWriteTransaction, MdbTxn,
    OwnedMdbVal, MDB_SUCCESS,
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

    pub fn put(
        &self,
        transaction: &dyn WriteTransaction,
        account: &Account,
        info: &AccountInfo,
    ) -> Result<()> {
        let mut account_val = OwnedMdbVal::try_from(account)?;
        let mut info_val = OwnedMdbVal::try_from(info)?;

        let status = unsafe {
            mdb_put(
                get_raw_lmdb_txn(transaction.as_transaction()),
                self.accounts_handle,
                account_val.as_mdb_val(),
                info_val.as_mdb_val(),
                0,
            )
        };
        ensure_success(status)
    }
}

fn get_raw_lmdb_txn(txn: &dyn Transaction) -> *mut MdbTxn {
    let any = txn.as_any();
    if let Some(t) = any.downcast_ref::<LmdbReadTransaction>() {
        t.handle.clone()
    } else if let Some(t) = any.downcast_ref::<LmdbWriteTransaction>() {
        t.handle
    } else {
        panic!("not an LMDB transaction");
    }
}

impl TryFrom<&Account> for OwnedMdbVal {
    type Error = anyhow::Error;

    fn try_from(value: &Account) -> Result<Self, Self::Error> {
        let mut stream = MemoryStream::new();
        value.serialize(&mut stream)?;
        Ok(OwnedMdbVal::new(stream.to_vec()))
    }
}

impl TryFrom<&AccountInfo> for OwnedMdbVal {
    type Error = anyhow::Error;

    fn try_from(value: &AccountInfo) -> Result<Self, Self::Error> {
        let mut stream = MemoryStream::new();
        value.serialize(&mut stream)?;
        Ok(OwnedMdbVal::new(stream.to_vec()))
    }
}
