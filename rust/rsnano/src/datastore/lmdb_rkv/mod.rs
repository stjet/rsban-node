mod account_store;
mod block_store;
mod confirmation_height_store;
mod final_vote_store;
mod frontier_store;
mod iterator;
mod lmdb_env;
mod online_weight_store;
mod peer_store;
mod pending_store;
mod pruned_store;
mod unchecked_store;
mod version_store;
mod wallet_store;

pub use account_store::LmdbAccountStore;
pub use block_store::LmdbBlockStore;
pub use confirmation_height_store::LmdbConfirmationHeightStore;
pub use final_vote_store::LmdbFinalVoteStore;
pub use frontier_store::LmdbFrontierStore;
pub use iterator::LmdbIteratorImpl;
use lmdb::{
    Database, Environment, InactiveTransaction, RoCursor, RoTransaction, RwTransaction, Transaction,
};
pub use lmdb_env::LmdbEnv;
#[cfg(test)]
pub(crate) use lmdb_env::TestLmdbEnv;
pub use online_weight_store::LmdbOnlineWeightStore;
pub use peer_store::LmdbPeerStore;
pub use pending_store::LmdbPendingStore;
pub use pruned_store::LmdbPrunedStore;
use std::{mem, sync::Arc};
pub use unchecked_store::LmdbUncheckedStore;
pub use wallet_store::LmdbWalletStore;

use super::TxnCallbacks;

enum RoTxnState<'a> {
    Inactive(InactiveTransaction<'a>),
    Active(RoTransaction<'a>),
    Transitioning,
}

pub struct LmdbReadTransaction<'a> {
    txn_id: u64,
    callbacks: Arc<dyn TxnCallbacks>,
    txn: RoTxnState<'a>,
}

impl<'a> LmdbReadTransaction<'a> {
    pub fn new(
        txn_id: u64,
        env: &'a Environment,
        callbacks: Arc<dyn TxnCallbacks>,
    ) -> lmdb::Result<Self> {
        let txn = env.begin_ro_txn()?;
        callbacks.txn_start(txn_id, false);

        Ok(Self {
            txn_id,
            callbacks,
            txn: RoTxnState::Active(txn),
        })
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

    pub fn refresh(&mut self) {
        self.reset();
        self.renew();
    }

    pub fn txn(&self) -> &lmdb::RoTransaction {
        match &self.txn {
            RoTxnState::Active(t) => t,
            _ => panic!("LMDB read transaction not active"),
        }
    }

    pub fn as_txn(&self) -> LmdbTransaction {
        LmdbTransaction::Read(self)
    }
}

impl<'a> Drop for LmdbReadTransaction<'a> {
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

enum RwTxnState<'a> {
    Inactive(),
    Active(RwTransaction<'a>),
    Transitioning,
}

pub struct LmdbWriteTransaction<'a> {
    env: &'a Environment,
    txn_id: u64,
    callbacks: Arc<dyn TxnCallbacks>,
    txn: RwTxnState<'a>,
}

impl<'a> LmdbWriteTransaction<'a> {
    pub fn new(
        txn_id: u64,
        env: &'a Environment,
        callbacks: Arc<dyn TxnCallbacks>,
    ) -> lmdb::Result<Self> {
        let mut tx = Self {
            env,
            txn_id,
            callbacks,
            txn: RwTxnState::Inactive(),
        };
        tx.renew();
        Ok(tx)
    }

    pub fn as_txn(&self) -> LmdbTransaction {
        LmdbTransaction::Write(self)
    }

    pub fn rw_txn(&self) -> &RwTransaction<'a> {
        match &self.txn {
            RwTxnState::Active(t) => t,
            _ => panic!("txn not active"),
        }
    }

    pub fn rw_txn_mut(&mut self) -> &mut RwTransaction<'a> {
        match &mut self.txn {
            RwTxnState::Active(t) => t,
            _ => panic!("txn not active"),
        }
    }

    pub fn commit(&mut self) {
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

    pub fn renew(&mut self) {
        let t = mem::replace(&mut self.txn, RwTxnState::Transitioning);
        self.txn = match t {
            RwTxnState::Active(_) => panic!("Cannot renew active RwTransaction"),
            RwTxnState::Inactive() => RwTxnState::Active(self.env.begin_rw_txn().unwrap()),
            RwTxnState::Transitioning => unreachable!(),
        };
        self.callbacks.txn_start(self.txn_id, true);
    }

    pub fn refresh(&mut self) {
        self.commit();
        self.renew();
    }
}

impl<'a> Drop for LmdbWriteTransaction<'a> {
    fn drop(&mut self) {
        self.commit();
    }
}

pub type LmdbTransaction<'a> =
    super::Transaction<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>>;

impl<'a> LmdbTransaction<'a> {
    fn get<'txn, K>(&'txn self, database: lmdb::Database, key: &K) -> lmdb::Result<&'txn [u8]>
    where
        K: AsRef<[u8]>,
    {
        match self {
            super::Transaction::Read(r) => r.txn().get(database, key),
            super::Transaction::Write(w) => w.rw_txn().get(database, key),
        }
    }

    fn open_ro_cursor<'txn>(&'txn self, database: Database) -> lmdb::Result<RoCursor> {
        match self {
            super::Transaction::Read(r) => r.txn().open_ro_cursor(database),
            super::Transaction::Write(w) => w.rw_txn().open_ro_cursor(database),
        }
    }

    fn count(&self, database: Database) -> usize {
        let stats = match self {
            super::Transaction::Read(r) => r.txn().stat(database),
            super::Transaction::Write(w) => w.rw_txn().stat(database),
        };

        stats.unwrap().entries()
    }
}

pub fn exists(txn: &LmdbTransaction, db: Database, key: &[u8]) -> bool {
    match txn.get(db, &key) {
        Ok(_) => true,
        Err(lmdb::Error::NotFound) => false,
        Err(e) => panic!("exists failed: {:?}", e),
    }
}
