use std::{
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex,
    },
};

use crate::{
    datastore::Transaction,
    utils::{Deserialize, Serialize, Stream, StreamExt},
    Account, Fan, RawKey,
};

use super::{
    assert_success, ensure_success, get_raw_lmdb_txn, mdb_dbi_open, mdb_put, OwnedMdbVal,
    MDB_CREATE,
};

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

pub struct WalletValue {
    pub key: RawKey,
    pub work: u64,
}

impl WalletValue {
    pub fn new(key: RawKey, work: u64) -> Self {
        Self { key, work }
    }
}

impl Serialize for WalletValue {
    fn serialized_size() -> usize {
        RawKey::serialized_size()
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        self.key.serialize(stream)?;
        stream.write_u64_ne(self.work)
    }
}

impl Deserialize for WalletValue {
    type Target = Self;

    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self::Target> {
        let key = RawKey::deserialize(stream)?;
        let work = stream.read_u64_ne()?;
        Ok(WalletValue::new(key, work))
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

    pub fn entry_put_raw(&self, txn: &dyn Transaction, account: &Account, entry: &WalletValue) {
        let mut key = OwnedMdbVal::from(account);
        let mut value = OwnedMdbVal::from(entry);
        let status = unsafe {
            mdb_put(
                get_raw_lmdb_txn(txn),
                self.db_handle(),
                key.as_mdb_val(),
                value.as_mdb_val(),
                0,
            )
        };
        assert_success(status);
    }
}
