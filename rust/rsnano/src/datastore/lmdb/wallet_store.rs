use std::{
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex,
    },
};

use crate::{datastore::Transaction, Fan, RawKey};

use super::{ensure_success, get_raw_lmdb_txn, mdb_dbi_open, MDB_CREATE};

pub struct Fans {
    pub password: Fan,
    pub wallet_key_mem: Fan,
}

impl Fans {
    pub fn new(fanout: usize) -> Self {
        Self {
            password: Fan::new(RawKey::new(), fanout),
            wallet_key_mem: Fan::new(RawKey::new(), fanout),
        }
    }
}

pub struct LmdbWalletStore {
    db_handle: AtomicU32,
    pub fans: Mutex<Fans>,
}

impl LmdbWalletStore {
    pub fn new(fanout: usize) -> Self {
        Self {
            db_handle: AtomicU32::new(0),
            fans: Mutex::new(Fans::new(fanout)),
        }
    }

    pub fn initialize(&self, txn: &dyn Transaction, path: &Path) -> anyhow::Result<()> {
        let path_str = path
            .as_os_str()
            .to_str()
            .ok_or_else(|| anyhow!("invalid path"))?;
        let mut handle = 0;
        let status =
            unsafe { mdb_dbi_open(get_raw_lmdb_txn(txn), path_str, MDB_CREATE, &mut handle) };
        self.db_handle.store(handle, Ordering::SeqCst);
        ensure_success(status)
    }

    pub fn db_handle(&self) -> u32 {
        self.db_handle.load(Ordering::SeqCst)
    }

    pub fn set_db_handle(&self, handle: u32) {
        self.db_handle.store(handle, Ordering::SeqCst);
    }
}
