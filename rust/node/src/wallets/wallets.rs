use super::{Wallet, WalletActionThread, WalletRepresentatives};
use crate::{
    block_processing::{BlockProcessor, BlockSource},
    config::NodeConfig,
    utils::ThreadPool,
    work::DistributedWorkFactory,
    NetworkParams, OnlineReps,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::{
    utils::get_env_or_default_string, work::WorkThresholds, Account, Amount, BlockDetails,
    BlockEnum, BlockHash, HackyUnsafeMutBlock, KeyDerivationFunction, NoValue, PublicKey, RawKey,
    Root, WalletId, WorkVersion,
};
use rsnano_ledger::{BlockStatus, Ledger};
use rsnano_store_lmdb::{
    create_backup_file, BinaryDbIterator, DbIterator, Environment, EnvironmentWrapper, LmdbEnv,
    LmdbIteratorImpl, LmdbWalletStore, LmdbWriteTransaction, RwTransaction, Transaction,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::{info, warn};

#[derive(FromPrimitive)]
pub enum WalletsError {
    None,
    Generic,
    WalletNotFound,
    WalletLocked,
    AccountNotFound,
    InvalidPassword,
    BadPublicKey,
}

pub type WalletsIterator<T> = BinaryDbIterator<[u8; 64], NoValue, LmdbIteratorImpl<T>>;

pub struct Wallets<T: Environment + 'static = EnvironmentWrapper> {
    pub handle: Option<T::Database>,
    pub send_action_ids_handle: Option<T::Database>,
    enable_voting: bool,
    env: Arc<LmdbEnv<T>>,
    pub mutex: Mutex<HashMap<WalletId, Arc<Wallet<T>>>>,
    pub node_config: NodeConfig,
    ledger: Arc<Ledger>,
    last_log: Mutex<Option<Instant>>,
    distributed_work: Arc<DistributedWorkFactory>,
    work_thresholds: WorkThresholds,
    network_params: NetworkParams,
    pub delayed_work: Mutex<HashMap<Account, Root>>,
    workers: Arc<dyn ThreadPool>,
    pub wallet_actions: WalletActionThread<T>,
    block_processor: Arc<BlockProcessor>,
    pub representatives: Mutex<WalletRepresentatives>,
    online_reps: Arc<Mutex<OnlineReps>>,
    kdf: KeyDerivationFunction,
}

impl<T: Environment + 'static> Wallets<T> {
    pub fn new(
        enable_voting: bool,
        env: Arc<LmdbEnv<T>>,
        ledger: Arc<Ledger>,
        node_config: &NodeConfig,
        kdf_work: u32,
        work: WorkThresholds,
        distributed_work: Arc<DistributedWorkFactory>,
        network_params: NetworkParams,
        workers: Arc<dyn ThreadPool>,
        block_processor: Arc<BlockProcessor>,
        online_reps: Arc<Mutex<OnlineReps>>,
    ) -> anyhow::Result<Self> {
        let kdf = KeyDerivationFunction::new(kdf_work);
        let mut wallets = Self {
            handle: None,
            send_action_ids_handle: None,
            enable_voting,
            mutex: Mutex::new(HashMap::new()),
            env,
            node_config: node_config.clone(),
            ledger: Arc::clone(&ledger),
            last_log: Mutex::new(None),
            distributed_work,
            work_thresholds: work.clone(),
            network_params,
            delayed_work: Mutex::new(HashMap::new()),
            workers,
            wallet_actions: WalletActionThread::new(),
            block_processor,
            representatives: Mutex::new(WalletRepresentatives::new(
                node_config.vote_minimum,
                Arc::clone(&ledger),
            )),
            online_reps,
            kdf: kdf.clone(),
        };
        let mut txn = wallets.env.tx_begin_write();
        wallets.initialize(&mut txn)?;
        {
            let mut guard = wallets.mutex.lock().unwrap();
            let wallet_ids = wallets.get_wallet_ids(&txn);
            for id in wallet_ids {
                assert!(!guard.contains_key(&id));
                let representative = node_config.random_representative();
                let text = PathBuf::from(id.encode_hex());
                let wallet = Wallet::new(
                    Arc::clone(&ledger),
                    work.clone(),
                    &mut txn,
                    node_config.password_fanout as usize,
                    kdf.clone(),
                    representative,
                    &text,
                )?;

                guard.insert(id, Arc::new(wallet));
            }

            // Backup before upgrade wallets
            let mut backup_required = false;
            if node_config.backup_before_upgrade {
                let txn = wallets.env.tx_begin_read();
                for wallet in guard.values() {
                    if wallet.store.version(&txn) != LmdbWalletStore::<T>::VERSION_CURRENT {
                        backup_required = true;
                        break;
                    }
                }
            }
            if backup_required {
                create_backup_file(&wallets.env)?;
            }
            // TODO port more here...
        }

        Ok(wallets)
    }

    pub fn start(&self) {
        self.wallet_actions.start();
    }

    pub fn stop(&self) {
        self.wallet_actions.stop();
    }

    pub fn initialize(&mut self, txn: &mut LmdbWriteTransaction<T>) -> anyhow::Result<()> {
        self.handle = Some(unsafe { txn.rw_txn_mut().create_db(None, DatabaseFlags::empty())? });
        self.send_action_ids_handle = Some(unsafe {
            txn.rw_txn_mut()
                .create_db(Some("send_action_ids"), DatabaseFlags::empty())?
        });
        Ok(())
    }

    pub fn get_store_it(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &str,
    ) -> WalletsIterator<T> {
        let hash_bytes: [u8; 64] = hash.as_bytes().try_into().unwrap();
        WalletsIterator::new(LmdbIteratorImpl::<T>::new(
            txn,
            self.handle.unwrap(),
            Some(&hash_bytes),
            true,
        ))
    }

    pub fn get_wallet_ids(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> Vec<WalletId> {
        let mut wallet_ids = Vec::new();
        let beginning = RawKey::from(0).encode_hex();
        let mut i = self.get_store_it(txn, &beginning);
        while let Some((k, _)) = i.current() {
            let text = std::str::from_utf8(k).unwrap();
            wallet_ids.push(WalletId::decode_hex(text).unwrap());
            i.next();
        }
        wallet_ids
    }

    pub fn get_block_hash(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        id: &str,
    ) -> anyhow::Result<Option<BlockHash>> {
        match txn.get(self.send_action_ids_handle.unwrap(), id.as_bytes()) {
            Ok(bytes) => Ok(Some(
                BlockHash::from_slice(bytes).ok_or_else(|| anyhow!("invalid block hash"))?,
            )),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_block_hash(
        &self,
        txn: &mut LmdbWriteTransaction<T>,
        id: &str,
        hash: &BlockHash,
    ) -> anyhow::Result<()> {
        txn.rw_txn_mut().put(
            self.send_action_ids_handle.unwrap(),
            id.as_bytes(),
            hash.as_bytes(),
            WriteFlags::empty(),
        )?;
        Ok(())
    }

    pub fn clear_send_ids(&self, txn: &mut LmdbWriteTransaction<T>) {
        txn.clear_db(self.send_action_ids_handle.unwrap()).unwrap();
    }

    pub fn foreach_representative<F>(&self, mut action: F)
    where
        F: FnMut(&Account, &RawKey),
    {
        if self.node_config.enable_voting {
            let mut action_accounts_l: Vec<(PublicKey, RawKey)> = Vec::new();
            {
                let transaction_l = self.env.tx_begin_read();
                let ledger_txn = self.ledger.read_txn();
                let lock = self.mutex.lock().unwrap();
                for (wallet_id, wallet) in lock.iter() {
                    let representatives_l = wallet.representatives.lock().unwrap().clone();
                    for account in representatives_l {
                        if wallet.store.exists(&transaction_l, &account) {
                            if !self.ledger.weight_exact(&ledger_txn, account).is_zero() {
                                if wallet.store.valid_password(&transaction_l) {
                                    let prv = wallet
                                        .store
                                        .fetch(&transaction_l, &account)
                                        .expect("could not fetch account from wallet");

                                    action_accounts_l.push((account, prv));
                                } else {
                                    let mut last_log_guard = self.last_log.lock().unwrap();
                                    let should_log = match last_log_guard.as_ref() {
                                        Some(i) => i.elapsed() >= Duration::from_secs(60),
                                        None => true,
                                    };
                                    if should_log {
                                        *last_log_guard = Some(Instant::now());
                                        warn!("Representative locked inside wallet {}", wallet_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for (pub_key, prv_key) in action_accounts_l {
                action(&pub_key, &prv_key);
            }
        }
    }

    pub fn work_cache_blocking(&self, wallet: &Wallet<T>, account: &Account, root: &Root) {
        if self.distributed_work.work_generation_enabled() {
            let difficulty = self.work_thresholds.threshold_base(WorkVersion::Work1);
            if let Some(work) = self.distributed_work.make_blocking(
                WorkVersion::Work1,
                *root,
                difficulty,
                Some(*account),
            ) {
                let mut tx = self.env.tx_begin_write();
                if wallet.live() && wallet.store.exists(&tx, account) {
                    wallet.work_update(&mut tx, account, root, work);
                }
            } else {
                warn!(
                    "Could not precache work for root {} due to work generation failure",
                    root
                );
            }
        }
    }

    fn get_wallet<'a>(
        guard: &'a HashMap<WalletId, Arc<Wallet<T>>>,
        wallet_id: &WalletId,
    ) -> Result<&'a Arc<Wallet<T>>, WalletsError> {
        guard.get(wallet_id).ok_or(WalletsError::WalletNotFound)
    }

    pub fn insert_watch(
        &self,
        wallet_id: &WalletId,
        accounts: &[Account],
    ) -> Result<(), WalletsError> {
        let guard = self.mutex.lock().unwrap();
        let wallet = Self::get_wallet(&guard, wallet_id)?;
        let mut tx = self.env.tx_begin_write();
        if !wallet.store.valid_password(&tx) {
            return Err(WalletsError::WalletLocked);
        }

        for account in accounts {
            if wallet.store.insert_watch(&mut tx, account).is_err() {
                return Err(WalletsError::BadPublicKey);
            }
        }

        Ok(())
    }

    pub fn valid_password(&self, wallet_id: &WalletId) -> Result<bool, WalletsError> {
        let guard = self.mutex.lock().unwrap();
        let wallet = Self::get_wallet(&guard, wallet_id)?;
        let tx = self.env.tx_begin_read();
        Ok(wallet.store.valid_password(&tx))
    }

    pub fn attempt_password(
        &self,
        wallet_id: &WalletId,
        password: impl AsRef<str>,
    ) -> Result<(), WalletsError> {
        let guard = self.mutex.lock().unwrap();
        let wallet = Self::get_wallet(&guard, wallet_id)?;
        let tx = self.env.tx_begin_write();
        if wallet.store.attempt_password(&tx, password.as_ref()) {
            Ok(())
        } else {
            Err(WalletsError::InvalidPassword)
        }
    }

    pub fn lock(&self, wallet_id: &WalletId) -> Result<(), WalletsError> {
        let guard = self.mutex.lock().unwrap();
        let wallet = Self::get_wallet(&guard, wallet_id)?;
        wallet.store.lock();
        Ok(())
    }

    pub fn rekey(
        &self,
        wallet_id: &WalletId,
        password: impl AsRef<str>,
    ) -> Result<(), WalletsError> {
        let guard = self.mutex.lock().unwrap();
        let wallet = Self::get_wallet(&guard, wallet_id)?;
        let mut tx = self.env.tx_begin_write();
        if !wallet.store.valid_password(&tx) {
            return Err(WalletsError::WalletLocked);
        }

        wallet
            .store
            .rekey(&mut tx, password.as_ref())
            .map_err(|_| WalletsError::Generic)
    }

    pub fn set_observer(&self, observer: Box<dyn Fn(bool) + Send>) {
        self.wallet_actions.set_observer(observer);
    }

    pub fn compute_reps(&self) {
        let wallets_guard = self.mutex.lock().unwrap();
        let mut reps_guard = self.representatives.lock().unwrap();
        reps_guard.clear();
        let half_principal_weight = self.online_reps.lock().unwrap().minimum_principal_weight() / 2;
        let tx = self.env.tx_begin_read();
        for (_, wallet) in wallets_guard.iter() {
            let mut representatives = HashSet::new();
            let mut it = wallet.store.begin(&tx);
            while let Some((&account, _)) = it.current() {
                if reps_guard.check_rep(account, half_principal_weight) {
                    representatives.insert(account);
                }
                it.next();
            }
            *wallet.representatives.lock().unwrap() = representatives;
        }
    }

    pub fn exists(&self, account: &Account) -> bool {
        let guard = self.mutex.lock().unwrap();
        let tx = self.env.tx_begin_read();
        guard
            .values()
            .any(|wallet| wallet.store.exists(&tx, account))
    }

    pub fn reload(&self) {
        let mut guard = self.mutex.lock().unwrap();
        let mut tx = self.env.tx_begin_write();
        let mut stored_items = HashSet::new();
        let wallet_ids = self.get_wallet_ids(&tx);
        for id in wallet_ids {
            // New wallet
            if !guard.contains_key(&id) {
                let text = PathBuf::from(id.encode_hex());
                let representative = self.node_config.random_representative();
                if let Ok(wallet) = Wallet::new(
                    Arc::clone(&self.ledger),
                    self.work_thresholds.clone(),
                    &mut tx,
                    self.node_config.password_fanout as usize,
                    self.kdf.clone(),
                    representative,
                    &text,
                ) {
                    guard.insert(id, Arc::new(wallet));
                }
            }
            // List of wallets on disk
            stored_items.insert(id);
        }
        // Delete non existing wallets from memory
        let mut deleted_items = Vec::new();
        for &id in guard.keys() {
            if !stored_items.contains(&id) {
                deleted_items.push(id);
            }
        }
        for i in &deleted_items {
            guard.remove(i);
        }
    }

    pub fn destroy(&self, id: &WalletId) {
        let mut guard = self.mutex.lock().unwrap();
        let mut tx = self.env.tx_begin_write();
        // action_mutex should be locked after transactions to prevent deadlocks in deterministic_insert () & insert_adhoc ()
        let _action_guard = self.wallet_actions.lock_safe();
        let wallet = guard.remove(id).unwrap();
        wallet.store.destroy(&mut tx);
    }

    pub fn remove_account(
        &self,
        wallet_id: &WalletId,
        account: &Account,
    ) -> Result<(), WalletsError> {
        let guard = self.mutex.lock().unwrap();
        let wallet = Self::get_wallet(&guard, wallet_id)?;
        let mut tx = self.env.tx_begin_write();
        if !wallet.store.valid_password(&tx) {
            return Err(WalletsError::WalletLocked);
        }
        if wallet.store.find(&tx, account).is_end() {
            return Err(WalletsError::AccountNotFound);
        }
        wallet.store.erase(&mut tx, account);
        Ok(())
    }

    pub fn work_set(
        &self,
        wallet_id: &WalletId,
        account: &Account,
        work: u64,
    ) -> Result<(), WalletsError> {
        let guard = self.mutex.lock().unwrap();
        let wallet = Self::get_wallet(&guard, wallet_id)?;
        let mut tx = self.env.tx_begin_write();
        if wallet.store.find(&tx, account).is_end() {
            return Err(WalletsError::AccountNotFound);
        }
        wallet.store.work_put(&mut tx, account, work);
        Ok(())
    }

    pub fn move_accounts(
        &self,
        source_id: &WalletId,
        target_id: &WalletId,
        accounts: &[Account],
    ) -> anyhow::Result<()> {
        let guard = self.mutex.lock().unwrap();
        let source = guard
            .get(source_id)
            .ok_or_else(|| anyhow!("source not found"))?;
        let mut tx = self.env.tx_begin_write();
        let target = guard
            .get(target_id)
            .ok_or_else(|| anyhow!("target not found"))?;
        target.store.move_keys(&mut tx, &source.store, accounts)
    }
}

const GENERATE_PRIORITY: Amount = Amount::MAX;

pub trait WalletsExt<T: Environment = EnvironmentWrapper> {
    fn insert_adhoc(&self, wallet: &Arc<Wallet<T>>, key: &RawKey, generate_work: bool) -> Account;

    fn insert_adhoc2(
        &self,
        wallet_id: &WalletId,
        key: &RawKey,
        generate_work: bool,
    ) -> Result<Account, WalletsError>;

    fn work_ensure(&self, wallet: Arc<Wallet<T>>, account: Account, root: Root);

    fn action_complete(
        &self,
        wallet: Arc<Wallet<T>>,
        block: Option<Arc<BlockEnum>>,
        account: Account,
        generate_work: bool,
        details: &BlockDetails,
    ) -> anyhow::Result<()>;

    fn ongoing_compute_reps(&self);
}

impl<T: Environment> WalletsExt<T> for Arc<Wallets<T>> {
    fn insert_adhoc(&self, wallet: &Arc<Wallet<T>>, key: &RawKey, generate_work: bool) -> Account {
        let mut tx = self.env.tx_begin_write();
        if !wallet.store.valid_password(&tx) {
            return PublicKey::zero();
        }
        let key = wallet.store.insert_adhoc(&mut tx, key);
        let block_tx = self.ledger.read_txn();
        if generate_work {
            self.work_ensure(
                Arc::clone(wallet),
                key,
                self.ledger.latest_root(&block_tx, &key),
            );
        }
        let half_principal_weight = self.online_reps.lock().unwrap().minimum_principal_weight() / 2;
        // Makes sure that the representatives container will
        // be in sync with any added keys.
        tx.commit();
        let mut rep_guard = self.representatives.lock().unwrap();
        if rep_guard.check_rep(key, half_principal_weight) {
            wallet.representatives.lock().unwrap().insert(key);
        }
        key
    }

    fn insert_adhoc2(
        &self,
        wallet_id: &WalletId,
        key: &RawKey,
        generate_work: bool,
    ) -> Result<Account, WalletsError> {
        let guard = self.mutex.lock().unwrap();
        let wallet = Wallets::get_wallet(&guard, wallet_id)?;
        let mut tx = self.env.tx_begin_read();
        if !wallet.store.valid_password(&tx) {
            return Err(WalletsError::WalletLocked);
        }
        tx.reset();
        Ok(self.insert_adhoc(wallet, key, generate_work))
    }

    fn work_ensure(&self, wallet: Arc<Wallet<T>>, account: Account, root: Root) {
        let precache_delay = if self.network_params.network.is_dev_network() {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(10)
        };
        self.delayed_work.lock().unwrap().insert(account, root);
        let self_clone = Arc::clone(self);
        self.workers.add_delayed_task(
            precache_delay,
            Box::new(move || {
                let mut guard = self_clone.delayed_work.lock().unwrap();
                if let Some(&existing) = guard.get(&account) {
                    if existing == root {
                        guard.remove(&account);
                        let self_clone_2 = Arc::clone(&self_clone);
                        self_clone.wallet_actions.queue_wallet_action(
                            GENERATE_PRIORITY,
                            wallet,
                            Box::new(move |w| {
                                self_clone_2.work_cache_blocking(&w, &account, &root);
                            }),
                        );
                    }
                }
            }),
        );
    }

    fn action_complete(
        &self,
        wallet: Arc<Wallet<T>>,
        block: Option<Arc<BlockEnum>>,
        account: Account,
        generate_work: bool,
        details: &BlockDetails,
    ) -> anyhow::Result<()> {
        // Unschedule any work caching for this account
        self.delayed_work.lock().unwrap().remove(&account);
        let Some(block) = block else {
            return Ok(());
        };
        let hash = block.hash();
        let required_difficulty = self
            .network_params
            .work
            .threshold2(block.work_version(), details);
        let mut_block = unsafe { block.undefined_behavior_mut() };
        if self.network_params.work.difficulty_block(mut_block) < required_difficulty {
            info!(
                "Cached or provided work for block {} account {} is invalid, regenerating...",
                block.hash(),
                account.encode_account()
            );
            self.distributed_work
                .make_blocking_block(mut_block, required_difficulty)
                .ok_or_else(|| anyhow!("no work generated"))?;
        }
        let result = self.block_processor.add_blocking(block, BlockSource::Local);

        if !matches!(result, Some(BlockStatus::Progress)) {
            bail!("block processor failed: {:?}", result);
        }

        if generate_work {
            // Pregenerate work for next block based on the block just created
            self.work_ensure(wallet, account, hash.into());
        }
        Ok(())
    }

    fn ongoing_compute_reps(&self) {
        self.compute_reps();

        // Representation drifts quickly on the test network but very slowly on the live network
        let compute_delay = if self.network_params.network.is_dev_network() {
            Duration::from_millis(10)
        } else if self.network_params.network.is_test_network() {
            test_scan_wallet_reps_delay()
        } else {
            Duration::from_secs(60 * 15)
        };

        let self_l = Arc::clone(self);
        self.workers.add_delayed_task(
            compute_delay,
            Box::new(move || {
                self_l.ongoing_compute_reps();
            }),
        );
    }
}

fn test_scan_wallet_reps_delay() -> Duration {
    let test_env = get_env_or_default_string("NANO_TEST_WALLET_SCAN_REPS_DELAY", "900000"); // 15 minutes by default
    Duration::from_millis(test_env.parse().unwrap())
}
