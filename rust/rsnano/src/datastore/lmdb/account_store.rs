use crate::datastore::Transaction;
use anyhow::Result;

use super::{mdb_dbi_open, LmdbReadTransaction, LmdbWriteTransaction, MdbTxn, MDB_SUCCESS};

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
