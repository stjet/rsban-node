use std::{
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex,
    },
};

use crate::{
    datastore::Transaction,
    utils::{Deserialize, Serialize, Stream, StreamAdapter, StreamExt},
    Account, Fan, RawKey,
};

use super::{
    assert_success, ensure_success, get_raw_lmdb_txn, mdb_dbi_open, mdb_get, mdb_put, MdbVal,
    OwnedMdbVal, MDB_CREATE, MDB_SUCCESS,
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

impl TryFrom<&MdbVal> for WalletValue {
    type Error = anyhow::Error;

    fn try_from(value: &MdbVal) -> Result<Self, Self::Error> {
        let mut stream = StreamAdapter::new(value.as_slice());
        WalletValue::deserialize(&mut stream)
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

    /// Random number used to salt private key encryption
    pub fn salt_special() -> Account {
        Account::from(1)
    }

    /// Check value used to see if password is valid
    pub fn check_special() -> Account {
        Account::from(3)
    }

    /// Wallet seed for deterministic key generation
    pub fn seed_special() -> Account {
        Account::from(5)
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

    pub fn entry_get_raw(&self, txn: &dyn Transaction, account: &Account) -> WalletValue {
        let mut key = MdbVal::from(account);
        let mut value = MdbVal::new();
        let status = unsafe {
            mdb_get(
                get_raw_lmdb_txn(txn),
                self.db_handle(),
                &mut key,
                &mut value,
            )
        };
        if status == MDB_SUCCESS {
            WalletValue::try_from(&value).unwrap()
        } else {
            WalletValue::new(RawKey::new(), 0)
        }
    }

    pub fn entry_put_raw(&self, txn: &dyn Transaction, account: &Account, entry: &WalletValue) {
        let mut key = MdbVal::from(account);
        let mut value = OwnedMdbVal::from(entry);
        let status = unsafe {
            mdb_put(
                get_raw_lmdb_txn(txn),
                self.db_handle(),
                &mut key,
                value.as_mdb_val(),
                0,
            )
        };
        assert_success(status);
    }

    pub fn check(&self, txn: &dyn Transaction) -> RawKey {
        self.entry_get_raw(txn, &Self::check_special()).key
    }

    pub fn salt(&self, txn: &dyn Transaction) -> RawKey {
        self.entry_get_raw(txn, &Self::salt_special()).key
    }

    pub fn wallet_key(&self, txn: &dyn Transaction) -> RawKey {
        let guard = self.fans.lock().unwrap();
        let wallet_l = guard.wallet_key_mem.value();
        let password_l = guard.password.value();
        let iv = self.salt(txn).initialization_vector_low();
        wallet_l.decrypt(&password_l, &iv)
    }

    pub fn seed(&self, txn: &dyn Transaction) -> RawKey {
        let value = self.entry_get_raw(txn, &Self::seed_special());
        let password_l = self.wallet_key(txn);
        let iv = self.salt(txn).initialization_vector_high();
        value.key.decrypt(&password_l, &iv)
    }
}
