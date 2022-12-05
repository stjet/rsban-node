#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate anyhow;

mod iterator;
pub use iterator::LmdbIteratorImpl;

mod lmdb_config;
pub use lmdb_config::{LmdbConfig, SyncStrategy};

mod lmdb_env;
pub use lmdb_env::{EnvOptions, LmdbEnv, TestDbFile, TestLmdbEnv};

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

mod unchecked_store;
pub use unchecked_store::LmdbUncheckedStore;

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
    cmp::{max, min},
    mem,
    sync::Arc,
};

use lmdb::{
    Database, Environment, InactiveTransaction, RoCursor, RoTransaction, RwTransaction, Transaction,
};
use primitive_types::{U256, U512};
use rsnano_core::utils::get_cpu_count;
use rsnano_store_traits::{ReadTransaction, TransactionTracker, WriteTransaction};

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
    pub fn new<'a>(
        txn_id: u64,
        env: &'a Environment,
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
}

impl Drop for LmdbReadTransaction {
    fn drop(&mut self) {
        let t = mem::replace(&mut self.txn, RoTxnState::Transitioning);
        // This uses commit rather than abort, as it is needed when opening databases with a read only transaction
        match t {
            RoTxnState::Active(t) => t.commit().unwrap(),
            _ => {}
        }
        self.callbacks.txn_end(self.txn_id, false);
    }
}

impl rsnano_store_traits::Transaction for LmdbReadTransaction {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl ReadTransaction for LmdbReadTransaction {
    fn txn(&self) -> &dyn rsnano_store_traits::Transaction {
        self
    }

    fn reset(&mut self) {
        let t = mem::replace(&mut self.txn, RoTxnState::Transitioning);
        self.txn = match t {
            RoTxnState::Active(t) => RoTxnState::Inactive(t.reset()),
            RoTxnState::Inactive(_) => panic!("Cannot reset inactive transaction"),
            RoTxnState::Transitioning => unreachable!(),
        };
        self.callbacks.txn_end(self.txn_id, false);
    }

    fn renew(&mut self) {
        let t = mem::replace(&mut self.txn, RoTxnState::Transitioning);
        self.txn = match t {
            RoTxnState::Active(_) => panic!("Cannot renew active transaction"),
            RoTxnState::Inactive(t) => RoTxnState::Active(t.renew().unwrap()),
            RoTxnState::Transitioning => unreachable!(),
        };
        self.callbacks.txn_start(self.txn_id, false);
    }

    fn refresh(&mut self) {
        self.reset();
        self.renew();
    }
}

enum RwTxnState<'a> {
    Inactive(),
    Active(RwTransaction<'a>),
    Transitioning,
}

pub struct LmdbWriteTransaction {
    env: &'static Environment,
    txn_id: u64,
    callbacks: Arc<dyn TransactionTracker>,
    txn: RwTxnState<'static>,
}

impl LmdbWriteTransaction {
    pub fn new<'a>(
        txn_id: u64,
        env: &'a Environment,
        callbacks: Arc<dyn TransactionTracker>,
    ) -> lmdb::Result<Self> {
        let env = unsafe { std::mem::transmute::<&'a Environment, &'static Environment>(env) };
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
}

impl<'a> Drop for LmdbWriteTransaction {
    fn drop(&mut self) {
        self.commit();
    }
}

impl rsnano_store_traits::Transaction for LmdbWriteTransaction {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl WriteTransaction for LmdbWriteTransaction {
    fn txn(&self) -> &dyn rsnano_store_traits::Transaction {
        self
    }
    fn txn_mut(&mut self) -> &mut dyn rsnano_store_traits::Transaction {
        self
    }

    fn renew(&mut self) {
        let t = mem::replace(&mut self.txn, RwTxnState::Transitioning);
        self.txn = match t {
            RwTxnState::Active(_) => panic!("Cannot renew active RwTransaction"),
            RwTxnState::Inactive() => RwTxnState::Active(self.env.begin_rw_txn().unwrap()),
            RwTxnState::Transitioning => unreachable!(),
        };
        self.callbacks.txn_start(self.txn_id, true);
    }

    fn refresh(&mut self) {
        self.commit();
        self.renew();
    }

    fn commit(&mut self) {
        let t = mem::replace(&mut self.txn, RwTxnState::Transitioning);
        match t {
            RwTxnState::Inactive() => {}
            RwTxnState::Active(t) => {
                t.commit().unwrap();
                self.callbacks.txn_end(self.txn_id, true);
            }
            RwTxnState::Transitioning => unreachable!(),
        };
        self.txn = RwTxnState::Inactive();
    }
}

pub fn exists(txn: &dyn rsnano_store_traits::Transaction, db: Database, key: &[u8]) -> bool {
    match get(txn, db, &key) {
        Ok(_) => true,
        Err(lmdb::Error::NotFound) => false,
        Err(e) => panic!("exists failed: {:?}", e),
    }
}

pub fn as_write_txn(txn: &mut dyn WriteTransaction) -> &mut RwTransaction<'static> {
    txn.txn_mut()
        .as_any_mut()
        .downcast_mut::<LmdbWriteTransaction>()
        .unwrap()
        .rw_txn_mut()
}

pub fn get<'a, K: AsRef<[u8]>>(
    txn: &'a dyn rsnano_store_traits::Transaction,
    database: Database,
    key: &K,
) -> lmdb::Result<&'a [u8]> {
    let any = txn.as_any();
    if let Some(t) = any.downcast_ref::<LmdbWriteTransaction>() {
        t.rw_txn().get(database, key)
    } else {
        any.downcast_ref::<LmdbReadTransaction>()
            .unwrap()
            .txn()
            .get(database, key)
    }
}

pub fn open_ro_cursor<'a>(
    txn: &'a dyn rsnano_store_traits::Transaction,
    database: Database,
) -> lmdb::Result<RoCursor<'a>> {
    let any = txn.as_any();
    if let Some(t) = any.downcast_ref::<LmdbWriteTransaction>() {
        t.rw_txn().open_ro_cursor(database)
    } else {
        any.downcast_ref::<LmdbReadTransaction>()
            .unwrap()
            .txn()
            .open_ro_cursor(database)
    }
}

pub fn count<'a>(txn: &'a dyn rsnano_store_traits::Transaction, database: Database) -> u64 {
    let any = txn.as_any();
    let stat = if let Some(t) = any.downcast_ref::<LmdbWriteTransaction>() {
        t.rw_txn().stat(database)
    } else {
        any.downcast_ref::<LmdbReadTransaction>()
            .unwrap()
            .txn()
            .stat(database)
    };
    stat.unwrap().entries() as u64
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
pub const STORE_VERSION_CURRENT: i32 = 21;
