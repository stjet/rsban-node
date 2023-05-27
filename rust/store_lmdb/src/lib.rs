#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate anyhow;

mod iterator;
pub use iterator::{BinaryDbIterator, DbIterator, DbIteratorImpl, LmdbIteratorImpl};

mod lmdb_config;
pub use lmdb_config::{LmdbConfig, SyncStrategy};

mod lmdb_env;
pub use lmdb_env::{
    EnvOptions, EnvironmentOptions, EnvironmentStrategy, EnvironmentWrapper, LmdbEnv, TestDbFile,
    TestLmdbEnv,
};

mod account_store;
pub use account_store::LmdbAccountStore;

mod block_store;
pub use block_store::LmdbBlockStore;

mod confirmation_height_store;
pub use confirmation_height_store::LmdbConfirmationHeightStore;

mod final_vote_store;
pub use final_vote_store::LmdbFinalVoteStore;

mod frontier_store;
pub use frontier_store::LmdbFrontierStore;

mod online_weight_store;
pub use online_weight_store::LmdbOnlineWeightStore;

mod pending_store;
pub use pending_store::LmdbPendingStore;

mod peer_store;
pub use peer_store::LmdbPeerStore;

mod pruned_store;
pub use pruned_store::LmdbPrunedStore;

mod version_store;
pub use version_store::LmdbVersionStore;

mod wallet_store;
pub use wallet_store::{Fans, LmdbWalletStore, WalletValue};

mod fan;
pub use fan::Fan;

mod wallets;
pub use wallets::LmdbWallets;

mod store;
pub use store::{create_backup_file, LmdbStore};

use std::{
    any::Any,
    cmp::{max, min},
    mem,
    sync::Arc,
    time::Duration,
};

use lmdb::{Database, InactiveTransaction, RoCursor, RoTransaction, RwTransaction};
use primitive_types::{U256, U512};
use rsnano_core::utils::{get_cpu_count, PropertyTreeWriter};

pub trait Transaction {
    fn as_any(&self) -> &dyn Any;
    fn refresh(&mut self);
    fn get(&self, database: Database, key: &[u8]) -> lmdb::Result<&[u8]>;
    fn exists(&self, db: Database, key: &[u8]) -> bool {
        match self.get(db, &key) {
            Ok(_) => true,
            Err(lmdb::Error::NotFound) => false,
            Err(e) => panic!("exists failed: {:?}", e),
        }
    }
    fn open_ro_cursor(&self, database: Database) -> lmdb::Result<RoCursor>;
    fn count(&self, database: Database) -> u64;
}

pub trait TransactionTracker: Send + Sync {
    fn txn_start(&self, txn_id: u64, is_write: bool);
    fn txn_end(&self, txn_id: u64, is_write: bool);
    fn serialize_json(
        &self,
        json: &mut dyn PropertyTreeWriter,
        min_read_time: Duration,
        min_write_time: Duration,
    ) -> anyhow::Result<()>;
}

pub struct NullTransactionTracker {}

impl NullTransactionTracker {
    pub fn new() -> Self {
        Self {}
    }
}

impl TransactionTracker for NullTransactionTracker {
    fn txn_start(&self, _txn_id: u64, _is_write: bool) {}

    fn txn_end(&self, _txn_id: u64, _is_write: bool) {}

    fn serialize_json(
        &self,
        _json: &mut dyn PropertyTreeWriter,
        _min_read_time: Duration,
        _min_write_time: Duration,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

enum RoTxnState {
    Inactive(InactiveTransaction<'static>),
    Active(RoTransaction<'static>),
    Transitioning,
}

pub struct LmdbReadTransaction {
    txn_id: u64,
    callbacks: Arc<dyn TransactionTracker>,
    txn: RoTxnState,
}

impl LmdbReadTransaction {
    pub fn new<'a, T: EnvironmentStrategy>(
        txn_id: u64,
        env: &'a T,
        callbacks: Arc<dyn TransactionTracker>,
    ) -> lmdb::Result<Self> {
        let txn = env.begin_ro_txn()?;
        let txn = unsafe { std::mem::transmute::<RoTransaction<'a>, RoTransaction<'static>>(txn) };
        callbacks.txn_start(txn_id, false);

        Ok(Self {
            txn_id,
            callbacks,
            txn: RoTxnState::Active(txn),
        })
    }

    pub fn txn(&self) -> &lmdb::RoTransaction {
        match &self.txn {
            RoTxnState::Active(t) => t,
            _ => panic!("LMDB read transaction not active"),
        }
    }

    pub fn reset(&mut self) {
        let t = mem::replace(&mut self.txn, RoTxnState::Transitioning);
        self.txn = match t {
            RoTxnState::Active(t) => RoTxnState::Inactive(t.reset()),
            RoTxnState::Inactive(_) => panic!("Cannot reset inactive transaction"),
            RoTxnState::Transitioning => unreachable!(),
        };
        self.callbacks.txn_end(self.txn_id, false);
    }

    pub fn renew(&mut self) {
        let t = mem::replace(&mut self.txn, RoTxnState::Transitioning);
        self.txn = match t {
            RoTxnState::Active(_) => panic!("Cannot renew active transaction"),
            RoTxnState::Inactive(t) => RoTxnState::Active(t.renew().unwrap()),
            RoTxnState::Transitioning => unreachable!(),
        };
        self.callbacks.txn_start(self.txn_id, false);
    }
}

impl Drop for LmdbReadTransaction {
    fn drop(&mut self) {
        let t = mem::replace(&mut self.txn, RoTxnState::Transitioning);
        // This uses commit rather than abort, as it is needed when opening databases with a read only transaction
        match t {
            RoTxnState::Active(t) => lmdb::Transaction::commit(t).unwrap(),
            _ => {}
        }
        self.callbacks.txn_end(self.txn_id, false);
    }
}

impl Transaction for LmdbReadTransaction {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn refresh(&mut self) {
        self.reset();
        self.renew();
    }

    fn get(&self, database: Database, key: &[u8]) -> lmdb::Result<&[u8]> {
        lmdb::Transaction::get(self.txn(), database, &&*key)
    }

    fn open_ro_cursor(&self, database: Database) -> lmdb::Result<RoCursor> {
        lmdb::Transaction::open_ro_cursor(self.txn(), database)
    }

    fn count(&self, database: Database) -> u64 {
        let stat = lmdb::Transaction::stat(self.txn(), database);
        stat.unwrap().entries() as u64
    }
}

enum RwTxnState<'a> {
    Inactive(),
    Active(RwTransaction<'a>),
    Transitioning,
}

pub struct LmdbWriteTransaction<T: EnvironmentStrategy + 'static = EnvironmentWrapper> {
    env: &'static T,
    txn_id: u64,
    callbacks: Arc<dyn TransactionTracker>,
    txn: RwTxnState<'static>,
}

impl<T: EnvironmentStrategy> LmdbWriteTransaction<T> {
    pub fn new<'a>(
        txn_id: u64,
        env: &'a T,
        callbacks: Arc<dyn TransactionTracker>,
    ) -> lmdb::Result<Self> {
        let env = unsafe { std::mem::transmute::<&'a T, &'static T>(env) };
        let mut tx = Self {
            env,
            txn_id,
            callbacks,
            txn: RwTxnState::Inactive(),
        };
        tx.renew();
        Ok(tx)
    }

    pub fn rw_txn(&self) -> &RwTransaction<'static> {
        match &self.txn {
            RwTxnState::Active(t) => t,
            _ => panic!("txn not active"),
        }
    }

    pub fn rw_txn_mut(&mut self) -> &mut RwTransaction<'static> {
        match &mut self.txn {
            RwTxnState::Active(t) => t,
            _ => panic!("txn not active"),
        }
    }

    pub fn renew(&mut self) {
        let t = mem::replace(&mut self.txn, RwTxnState::Transitioning);
        self.txn = match t {
            RwTxnState::Active(_) => panic!("Cannot renew active RwTransaction"),
            RwTxnState::Inactive() => RwTxnState::Active(self.env.begin_rw_txn().unwrap()),
            RwTxnState::Transitioning => unreachable!(),
        };
        self.callbacks.txn_start(self.txn_id, true);
    }

    pub fn commit(&mut self) {
        let t = mem::replace(&mut self.txn, RwTxnState::Transitioning);
        match t {
            RwTxnState::Inactive() => {}
            RwTxnState::Active(t) => {
                lmdb::Transaction::commit(t).unwrap();
                self.callbacks.txn_end(self.txn_id, true);
            }
            RwTxnState::Transitioning => unreachable!(),
        };
        self.txn = RwTxnState::Inactive();
    }
}

impl<'a, T: EnvironmentStrategy> Drop for LmdbWriteTransaction<T> {
    fn drop(&mut self) {
        self.commit();
    }
}

impl<T: EnvironmentStrategy> Transaction for LmdbWriteTransaction<T> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn refresh(&mut self) {
        self.commit();
        self.renew();
    }

    fn get(&self, database: Database, key: &[u8]) -> lmdb::Result<&[u8]> {
        lmdb::Transaction::get(self.rw_txn(), database, &&*key)
    }

    fn open_ro_cursor(&self, database: Database) -> lmdb::Result<RoCursor> {
        lmdb::Transaction::open_ro_cursor(self.rw_txn(), database)
    }

    fn count(&self, database: Database) -> u64 {
        let stat = lmdb::Transaction::stat(self.rw_txn(), database);
        stat.unwrap().entries() as u64
    }
}

pub enum Table {
    ConfirmationHeight,
}

pub fn parallel_traversal(action: &(impl Fn(U256, U256, bool) + Send + Sync)) {
    parallel_traversal_impl(U256::max_value(), action);
}

pub fn parallel_traversal_u512(action: &(impl Fn(U512, U512, bool) + Send + Sync)) {
    parallel_traversal_impl(U512::max_value(), action);
}

pub fn parallel_traversal_impl<T>(value_max: T, action: &(impl Fn(T, T, bool) + Send + Sync))
where
    T: std::ops::Div<usize, Output = T> + std::ops::Mul<usize, Output = T> + Send + Copy,
{
    // Between 10 and 40 threads, scales well even in low power systems as long as actions are I/O bound
    let thread_count = max(10, min(40, 11 * get_cpu_count()));
    let split: T = value_max / thread_count;

    std::thread::scope(|s| {
        for thread in 0..thread_count {
            let start = split * thread;
            let end = split * (thread + 1);
            let is_last = thread == thread_count - 1;

            std::thread::Builder::new()
                .name("DB par traversl".to_owned())
                .spawn_scoped(s, move || {
                    action(start, end, is_last);
                })
                .unwrap();
        }
    });
}

pub const STORE_VERSION_MINIMUM: i32 = 21;
pub const STORE_VERSION_CURRENT: i32 = 22;
