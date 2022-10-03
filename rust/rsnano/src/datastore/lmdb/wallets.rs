use crate::datastore::Transaction;

use super::{ensure_success, get_raw_lmdb_txn, mdb_dbi_open, MDB_CREATE};

pub struct LmdbWallets {
    pub handle: u32,
}

impl LmdbWallets {
    pub fn new() -> Self {
        Self { handle: 0 }
    }

    pub fn initialize(&mut self, txn: &dyn Transaction) -> anyhow::Result<()> {
        let status =
            unsafe { mdb_dbi_open(get_raw_lmdb_txn(txn), None, MDB_CREATE, &mut self.handle) };
        ensure_success(status)
    }
}
