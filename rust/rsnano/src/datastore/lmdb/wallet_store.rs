use std::{
    fs::{set_permissions, File, Permissions},
    io::Write,
    os::unix::prelude::PermissionsExt,
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex, MutexGuard,
    },
};

use crate::{
    datastore::{DbIterator, NullIterator, Transaction},
    deterministic_key,
    ffi::create_ffi_property_tree,
    utils::{Deserialize, PropertyTreeWriter, Serialize, Stream, StreamAdapter, StreamExt},
    wallet::KeyDerivationFunction,
    Account, Fan, PublicKey, RawKey,
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

    /// Wallet version number
    pub fn version_special() -> Account {
        Account::from(0)
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

    /// Representative account to be used if we open a new account
    pub fn representative_special() -> Account {
        Account::from(4)
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
        self.deterministic_clear(txn);
    }

    pub fn deterministic_key(&self, txn: &dyn Transaction, index: u32) -> RawKey {
        debug_assert!(self.valid_password(txn));
        let seed = self.seed(txn);
        deterministic_key(&seed, index)
    }

    pub fn deterministic_index_get(&self, txn: &dyn Transaction) -> u32 {
        let value = self.entry_get_raw(txn, &Self::deterministic_index_special());
        value.key.number().low_u32()
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

    pub fn end(&self) -> Box<dyn DbIterator<Account, WalletValue>> {
        Box::new(NullIterator::new())
    }

    pub fn find(
        &self,
        txn: &dyn Transaction,
        account: &Account,
    ) -> Box<dyn DbIterator<Account, WalletValue>> {
        let result = self.begin_at_account(txn, account);
        if let Some((key, _)) = result.current() {
            if key == account {
                return result;
            }
        }

        self.end()
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

    pub fn valid_public_key(&self, key: &PublicKey) -> bool {
        key.number() >= Self::special_count().number()
    }

    pub fn exists(&self, txn: &dyn Transaction, key: &PublicKey) -> bool {
        self.valid_public_key(key) && !self.find(txn, &Account::from(key)).is_end()
    }

    pub fn deterministic_insert(&self, txn: &dyn Transaction) -> PublicKey {
        let mut index = self.deterministic_index_get(txn);
        let mut prv = self.deterministic_key(txn, index);
        let mut result = PublicKey::try_from(&prv).unwrap();
        while self.exists(txn, &result) {
            index += 1;
            prv = self.deterministic_key(txn, index);
            result = PublicKey::try_from(&prv).unwrap();
        }

        let mut marker = 1u64;
        marker <<= 32;
        marker |= index as u64;
        self.entry_put_raw(
            txn,
            &Account::from(result),
            &WalletValue::new(marker.into(), 0),
        );
        index += 1;
        self.deterministic_index_set(txn, index);
        return result;
    }

    pub fn deterministic_insert_at(&self, txn: &dyn Transaction, index: u32) -> PublicKey {
        let prv = self.deterministic_key(txn, index);
        let result = PublicKey::try_from(&prv).unwrap();
        let mut marker = 1u64;
        marker <<= 32;
        marker |= index as u64;
        self.entry_put_raw(txn, &result.into(), &&WalletValue::new(marker.into(), 0));
        result
    }

    pub fn version(&self, txn: &dyn Transaction) -> u32 {
        let value = self.entry_get_raw(txn, &Self::version_special());
        value.key.as_bytes()[31] as u32
    }

    pub fn attempt_password(&self, txn: &dyn Transaction, password: &str) -> bool {
        let is_valid = {
            let mut guard = self.fans.lock().unwrap();
            let password_key = self.derive_key(txn, password);
            guard.password.value_set(password_key);
            self.valid_password_locked(&guard, txn)
        };

        if is_valid {
            if self.version(txn) != 4 {
                panic!("invalid wallet store version!");
            }
        }

        is_valid
    }

    pub fn lock(&self) {
        self.fans.lock().unwrap().password.value_set(RawKey::new());
    }

    pub fn accounts(&self, txn: &dyn Transaction) -> Vec<Account> {
        let mut result = Vec::new();
        let mut it = self.begin(txn);
        while let Some((k, _)) = it.current() {
            result.push(*k);
            it.next();
        }

        result
    }

    pub fn representative(&self, txn: &dyn Transaction) -> Account {
        let value = self.entry_get_raw(txn, &Self::representative_special());
        Account::from_bytes(*value.key.as_bytes())
    }

    pub fn representative_set(&self, txn: &dyn Transaction, representative: &Account) {
        let rep = RawKey::from_bytes(*representative.as_bytes());
        self.entry_put_raw(
            txn,
            &Self::representative_special(),
            &&WalletValue::new(rep, 0),
        );
    }

    pub fn insert_adhoc(&self, txn: &dyn Transaction, prv: &RawKey) -> PublicKey {
        debug_assert!(self.valid_password(txn));
        let pub_key = PublicKey::try_from(prv).unwrap();
        let password = self.wallet_key(txn);
        let ciphertext = prv.encrypt(&password, &pub_key.initialization_vector());
        self.entry_put_raw(txn, &pub_key.into(), &WalletValue::new(ciphertext, 0));
        pub_key
    }

    pub fn insert_watch(&self, txn: &dyn Transaction, pub_key: &Account) -> anyhow::Result<()> {
        if !self.valid_public_key(&pub_key.public_key) {
            bail!("invalid public key");
        }

        self.entry_put_raw(txn, pub_key, &WalletValue::new(RawKey::new(), 0));
        Ok(())
    }

    pub fn fetch(&self, txn: &dyn Transaction, pub_key: &Account) -> anyhow::Result<RawKey> {
        if !self.valid_password(txn) {
            bail!("invalid password");
        }

        let value = self.entry_get_raw(txn, pub_key);
        if value.key.is_zero() {
            bail!("pub key not found");
        }

        let prv = match Self::key_type(&value) {
            KeyType::Deterministic => {
                let index = value.key.number().low_u32();
                self.deterministic_key(txn, index)
            }
            KeyType::Adhoc => {
                // Ad-hoc keys
                let password = self.wallet_key(txn);
                value
                    .key
                    .decrypt(&password, &pub_key.public_key.initialization_vector())
            }
            _ => bail!("invalid key type"),
        };

        let compare = PublicKey::try_from(&prv)?;
        if pub_key.public_key != compare {
            bail!("expected pub key does not match");
        }
        Ok(prv)
    }

    pub fn serialize_json(&self, txn: &dyn Transaction) -> String {
        let mut tree = create_ffi_property_tree();
        let mut it = LmdbIterator::<Account, WalletValue>::new(txn, self.db_handle(), None, true);

        while let Some((k, v)) = it.current() {
            tree.put_string(&k.encode_hex(), &v.key.encode_hex())
                .unwrap();
            it.next();
        }

        tree.to_json()
    }

    pub fn write_backup(&self, txn: &dyn Transaction, path: &Path) -> anyhow::Result<()> {
        let mut file = File::create(path)?;
        set_permissions(path, Permissions::from_mode(0o600))?;
        write!(file, "{}", self.serialize_json(txn))?;
        Ok(())
    }

    pub fn move_keys(
        &self,
        txn: &dyn Transaction,
        other: &LmdbWalletStore,
        keys: &[PublicKey],
    ) -> anyhow::Result<()> {
        debug_assert!(self.valid_password(txn));
        debug_assert!(other.valid_password(txn));
        for k in keys {
            let prv = other.fetch(txn, &k.into())?;
            self.insert_adhoc(txn, &prv);
            other.erase(txn, &k.into());
        }

        Ok(())
    }

    pub fn import(&self, txn: &dyn Transaction, other: &LmdbWalletStore) -> anyhow::Result<()> {
        debug_assert!(self.valid_password(txn));
        debug_assert!(other.valid_password(txn));
        let mut it = other.begin(txn);
        while let Some((k, _)) = it.current() {
            let prv = other.fetch(txn, k)?;
            if !prv.is_zero() {
                self.insert_adhoc(txn, &prv);
            } else {
                self.insert_watch(txn, k)?;
            }
            other.erase(txn, k);

            it.next();
        }

        Ok(())
    }

    pub fn work_get(&self, txn: &dyn Transaction, pub_key: &PublicKey) -> anyhow::Result<u64> {
        let entry = self.entry_get_raw(txn, &pub_key.into());
        if !entry.key.is_zero() {
            Ok(entry.work)
        } else {
            Err(anyhow!("not found"))
        }
    }
}
