use std::{
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex, MutexGuard,
    },
};

use crate::{
    datastore::{DbIterator, Transaction},
    utils::{Deserialize, Serialize, Stream, StreamAdapter, StreamExt},
    wallet::KeyDerivationFunction,
    Account, Fan, RawKey,
};

use super::{
    assert_success, ensure_success, get_raw_lmdb_txn, mdb_dbi_open, mdb_del, mdb_get, mdb_put,
    LmdbIterator, MdbVal, OwnedMdbVal, MDB_CREATE, MDB_SUCCESS,
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

#[derive(FromPrimitive)]
pub enum KeyType {
    NotAType,
    Unknown,
    Adhoc,
    Deterministic,
}

pub struct LmdbWalletStore {
    db_handle: AtomicU32,
    pub fans: Mutex<Fans>,
    kdf: KeyDerivationFunction,
}

impl LmdbWalletStore {
    pub fn new(fanout: usize, kdf: KeyDerivationFunction) -> Self {
        Self {
            db_handle: AtomicU32::new(0),
            fans: Mutex::new(Fans::new(fanout)),
            kdf,
        }
    }

    /// Random number used to salt private key encryption
    pub fn salt_special() -> Account {
        Account::from(1)
    }

    /// Key used to encrypt wallet keys, encrypted itself by the user password
    pub fn wallet_key_special() -> Account {
        Account::from(2)
    }

    /// Check value used to see if password is valid
    pub fn check_special() -> Account {
        Account::from(3)
    }

    /// Wallet seed for deterministic key generation
    pub fn seed_special() -> Account {
        Account::from(5)
    }

    /// Current key index for deterministic keys
    pub fn deterministic_index_special() -> Account {
        Account::from(6)
    }

    pub fn special_count() -> Account {
        Account::from(7)
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
        self.wallet_key_locked(&guard, txn)
    }

    fn wallet_key_locked(&self, guard: &MutexGuard<Fans>, txn: &dyn Transaction) -> RawKey {
        let wallet = guard.wallet_key_mem.value();
        let password = guard.password.value();
        let iv = self.salt(txn).initialization_vector_low();
        wallet.decrypt(&password, &iv)
    }

    pub fn seed(&self, txn: &dyn Transaction) -> RawKey {
        let value = self.entry_get_raw(txn, &Self::seed_special());
        let password = self.wallet_key(txn);
        let iv = self.salt(txn).initialization_vector_high();
        value.key.decrypt(&password, &iv)
    }

    pub fn set_seed(&self, txn: &dyn Transaction, prv: &RawKey) {
        let password_l = self.wallet_key(txn);
        let iv = self.salt(txn).initialization_vector_high();
        let ciphertext = prv.encrypt(&password_l, &iv);
        self.entry_put_raw(txn, &Self::seed_special(), &WalletValue::new(ciphertext, 0));
        //todo:
        //deterministic_clear (transaction_a);
    }

    pub fn deterministic_index_set(&self, txn: &dyn Transaction, index: u32) {
        let index = RawKey::from(index as u64);
        let value = WalletValue::new(index, 0);
        self.entry_put_raw(txn, &Self::deterministic_index_special(), &value);
    }

    pub fn valid_password(&self, txn: &dyn Transaction) -> bool {
        let wallet_key = self.wallet_key(txn);
        self.check_wallet_key(txn, &wallet_key)
    }

    pub fn valid_password_locked(&self, guard: &MutexGuard<Fans>, txn: &dyn Transaction) -> bool {
        let wallet_key = self.wallet_key_locked(guard, txn);
        self.check_wallet_key(txn, &wallet_key)
    }

    fn check_wallet_key(&self, txn: &dyn Transaction, wallet_key: &RawKey) -> bool {
        let zero = RawKey::new();
        let iv = self.salt(txn).initialization_vector_low();
        let check = zero.encrypt(&wallet_key, &iv);
        self.check(txn) == check
    }

    pub fn derive_key(&self, txn: &dyn Transaction, password: &str) -> RawKey {
        let salt = self.salt(txn);
        self.kdf.hash_password(password, salt.as_bytes())
    }

    pub fn rekey(&self, txn: &dyn Transaction, password: &str) -> anyhow::Result<()> {
        let mut guard = self.fans.lock().unwrap();
        if self.valid_password_locked(&guard, txn) {
            let password_new = self.derive_key(txn, password);
            let wallet_key = self.wallet_key_locked(&guard, txn);
            guard.password.value_set(password_new);
            let iv = self.salt(txn).initialization_vector_low();
            let encrypted = wallet_key.encrypt(&password_new, &iv);
            guard.wallet_key_mem.value_set(encrypted);
            self.entry_put_raw(
                txn,
                &Self::wallet_key_special(),
                &WalletValue::new(encrypted, 0),
            );
            Ok(())
        } else {
            Err(anyhow!("invalid password"))
        }
    }

    pub fn begin(&self, txn: &dyn Transaction) -> Box<dyn DbIterator<Account, WalletValue>> {
        Box::new(LmdbIterator::new(
            txn,
            self.db_handle(),
            Some(&Self::special_count()),
            true,
        ))
    }

    pub fn begin_at_account(
        &self,
        txn: &dyn Transaction,
        key: &Account,
    ) -> Box<dyn DbIterator<Account, WalletValue>> {
        Box::new(LmdbIterator::new(txn, self.db_handle(), Some(key), true))
    }

    pub fn erase(&self, txn: &dyn Transaction, account: &Account) {
        let status = unsafe {
            mdb_del(
                get_raw_lmdb_txn(txn),
                self.db_handle(),
                &mut MdbVal::from(account),
                None,
            )
        };
        assert_success(status);
    }

    pub fn key_type(value: &WalletValue) -> KeyType {
        let number = value.key.number();
        if number > u64::MAX.into() {
            KeyType::Adhoc
        } else {
            if (number >> 32).low_u32() == 1 {
                KeyType::Deterministic
            } else {
                KeyType::Unknown
            }
        }
    }

    pub fn deterministic_clear(&self, txn: &dyn Transaction) {
        let mut it = self.begin(txn);
        while let Some((account, value)) = it.current() {
            match Self::key_type(value) {
                KeyType::Deterministic => {
                    self.erase(txn, account);
                    it = self.begin_at_account(txn, account);
                }
                _ => it.next(),
            }
        }

        self.deterministic_index_set(txn, 0);
    }
}
