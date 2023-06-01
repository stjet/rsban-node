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
    EnvOptions, Environment, EnvironmentOptions, EnvironmentStub, EnvironmentWrapper, LmdbEnv,
    RoCursorWrapper, TestDbFile, TestLmdbEnv,
};
use lmdb_env::{InactiveTransaction, RoCursor, RoTransaction, RwTransaction};

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

use primitive_types::{U256, U512};
#[cfg(feature = "output_tracking")]
use rsnano_core::utils::OutputTracker;
use rsnano_core::utils::{get_cpu_count, OutputListener, PropertyTreeWriter};
#[cfg(feature = "output_tracking")]
use std::rc::Rc;

pub trait Transaction {
    type Database;
    type RoCursor: RoCursor;
    fn as_any(&self) -> &dyn Any;
    fn refresh(&mut self);
    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]>;
    fn exists(&self, db: Self::Database, key: &[u8]) -> bool {
        match self.get(db, &key) {
            Ok(_) => true,
            Err(lmdb::Error::NotFound) => false,
            Err(e) => panic!("exists failed: {:?}", e),
        }
    }
    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<Self::RoCursor>;
    fn count(&self, database: Self::Database) -> u64;
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

enum RoTxnState<T, U>
where
    T: RoTransaction<InactiveTxnType = U>,
    U: InactiveTransaction<RoTxnType = T>,
{
    Inactive(U),
    Active(T),
    Transitioning,
}

pub struct LmdbReadTransaction<T: Environment + 'static = EnvironmentWrapper> {
    txn_id: u64,
    callbacks: Arc<dyn TransactionTracker>,
    txn: RoTxnState<T::RoTxnImpl, T::InactiveTxnImpl>,
}

impl<T: Environment + 'static> LmdbReadTransaction<T> {
    pub fn new<'a>(
        txn_id: u64,
        env: &'a T,
        callbacks: Arc<dyn TransactionTracker>,
    ) -> lmdb::Result<Self> {
        let txn = env.begin_ro_txn()?;
        callbacks.txn_start(txn_id, false);

        Ok(Self {
            txn_id,
            callbacks,
            txn: RoTxnState::Active(txn),
        })
    }

    pub fn txn(&self) -> &T::RoTxnImpl {
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

impl<T: Environment + 'static> Drop for LmdbReadTransaction<T> {
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

impl<T: Environment + 'static> Transaction for LmdbReadTransaction<T> {
    type Database = T::Database;
    type RoCursor = T::RoCursor;

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn refresh(&mut self) {
        self.reset();
        self.renew();
    }

    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]> {
        self.txn().get(database, key)
    }

    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<Self::RoCursor> {
        self.txn().open_ro_cursor(database)
    }

    fn count(&self, database: Self::Database) -> u64 {
        self.txn().count(database)
    }
}

enum RwTxnState<T: RwTransaction> {
    Inactive,
    Active(T),
    Transitioning,
}

#[cfg(feature = "output_tracking")]
#[derive(Clone, Debug, PartialEq)]
pub struct PutEvent<T> {
    database: T,
    key: Vec<u8>,
    value: Vec<u8>,
    flags: lmdb::WriteFlags,
}

#[cfg(feature = "output_tracking")]
#[derive(Clone, Debug, PartialEq)]
pub struct DeleteEvent<T> {
    database: T,
    key: Vec<u8>,
}

pub struct LmdbWriteTransaction<T: Environment + 'static = EnvironmentWrapper> {
    env: &'static T,
    txn_id: u64,
    callbacks: Arc<dyn TransactionTracker>,
    txn: RwTxnState<T::RwTxnType>,
    #[cfg(feature = "output_tracking")]
    put_listener: OutputListener<PutEvent<T::Database>>,
    #[cfg(feature = "output_tracking")]
    delete_listener: OutputListener<DeleteEvent<T::Database>>,
}

impl<T: Environment> LmdbWriteTransaction<T> {
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
            txn: RwTxnState::Inactive,
            #[cfg(feature = "output_tracking")]
            put_listener: OutputListener::new(),
            #[cfg(feature = "output_tracking")]
            delete_listener: OutputListener::new(),
        };
        tx.renew();
        Ok(tx)
    }

    pub fn rw_txn(&self) -> &T::RwTxnType {
        match &self.txn {
            RwTxnState::Active(t) => t,
            _ => panic!("txn not active"),
        }
    }

    pub fn rw_txn_mut(&mut self) -> &mut T::RwTxnType {
        match &mut self.txn {
            RwTxnState::Active(t) => t,
            _ => panic!("txn not active"),
        }
    }

    pub fn renew(&mut self) {
        let t = mem::replace(&mut self.txn, RwTxnState::Transitioning);
        self.txn = match t {
            RwTxnState::Active(_) => panic!("Cannot renew active RwTransaction"),
            RwTxnState::Inactive => RwTxnState::Active(self.env.begin_rw_txn().unwrap()),
            RwTxnState::Transitioning => unreachable!(),
        };
        self.callbacks.txn_start(self.txn_id, true);
    }

    pub fn commit(&mut self) {
        let t = mem::replace(&mut self.txn, RwTxnState::Transitioning);
        match t {
            RwTxnState::Inactive => {}
            RwTxnState::Active(t) => {
                t.commit().unwrap();
                self.callbacks.txn_end(self.txn_id, true);
            }
            RwTxnState::Transitioning => unreachable!(),
        };
        self.txn = RwTxnState::Inactive;
    }

    #[cfg(feature = "output_tracking")]
    pub fn track_puts(&self) -> Rc<OutputTracker<PutEvent<T::Database>>> {
        self.put_listener.track()
    }

    #[cfg(feature = "output_tracking")]
    fn track_deletes(&self) -> Rc<OutputTracker<DeleteEvent<T::Database>>> {
        self.delete_listener.track()
    }

    unsafe fn create_db(
        &mut self,
        name: Option<&str>,
        flags: lmdb::DatabaseFlags,
    ) -> lmdb::Result<T::Database> {
        self.rw_txn().create_db(name, flags)
    }

    fn put(
        &mut self,
        database: T::Database,
        key: &[u8],
        value: &[u8],
        flags: lmdb::WriteFlags,
    ) -> lmdb::Result<()> {
        #[cfg(feature = "output_tracking")]
        self.put_listener.emit(PutEvent {
            database,
            key: key.to_vec(),
            value: value.to_vec(),
            flags,
        });
        self.rw_txn_mut().put(database, key, value, flags)
    }

    fn delete(
        &mut self,
        database: T::Database,
        key: &[u8],
        flags: Option<&[u8]>,
    ) -> lmdb::Result<()> {
        self.delete_listener.emit(DeleteEvent {
            database,
            key: key.to_vec(),
        });
        self.rw_txn_mut().del(database, key, flags)
    }
}

impl<'a, T: Environment> Drop for LmdbWriteTransaction<T> {
    fn drop(&mut self) {
        self.commit();
    }
}

impl<T: Environment> Transaction for LmdbWriteTransaction<T> {
    type Database = T::Database;
    type RoCursor = T::RoCursor;

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn refresh(&mut self) {
        self.commit();
        self.renew();
    }

    fn get(&self, database: Self::Database, key: &[u8]) -> lmdb::Result<&[u8]> {
        self.rw_txn().get(database, key)
    }

    fn open_ro_cursor(&self, database: Self::Database) -> lmdb::Result<Self::RoCursor> {
        self.rw_txn().open_ro_cursor(database)
    }

    fn count(&self, database: Self::Database) -> u64 {
        self.rw_txn().count(database)
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

#[cfg(test)]
mod test {
    use crate::lmdb_env::DatabaseStub;

    use super::*;

    #[test]
    fn tracks_deletes() {
        let env = LmdbEnv::create_null();
        let mut txn = env.tx_begin_write();
        let delete_tracker = txn.track_deletes();

        let database = DatabaseStub(42);
        let key = vec![1, 2, 3];
        txn.delete(database, &key, None).unwrap();

        assert_eq!(delete_tracker.output(), vec![DeleteEvent { database, key }])
    }
}
