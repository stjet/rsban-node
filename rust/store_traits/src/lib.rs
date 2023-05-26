mod iterator;
pub use iterator::{BinaryDbIterator, DbIterator, DbIteratorImpl};

mod block_store;
pub use block_store::{BlockIterator, BlockStore};

mod confirmation_height_store;
pub use confirmation_height_store::{ConfirmationHeightIterator, ConfirmationHeightStore};

mod final_vote_store;
pub use final_vote_store::{FinalVoteIterator, FinalVoteStore};

mod frontier_store;
pub use frontier_store::{FrontierIterator, FrontierStore};

mod online_weight_store;
pub use online_weight_store::{OnlineWeightIterator, OnlineWeightStore};

mod peer_store;
pub use peer_store::{PeerIterator, PeerStore};

mod pending_store;
pub use pending_store::{PendingIterator, PendingStore};

mod pruned_store;
pub use pruned_store::{PrunedIterator, PrunedStore};

use rsnano_core::{utils::PropertyTreeWriter, Account, AccountInfo};

mod version_store;
pub use version_store::VersionStore;

pub enum Table {
    ConfirmationHeight,
}

pub type AccountIterator = Box<dyn DbIterator<Account, AccountInfo>>;

use std::{any::Any, time::Duration};

pub trait Transaction {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn refresh(&mut self);
}

pub trait ReadTransaction {
    fn txn(&self) -> &dyn Transaction;
    fn txn_mut(&mut self) -> &mut dyn Transaction;
    fn reset(&mut self);
    fn renew(&mut self);
    fn refresh(&mut self);
}

pub trait WriteTransaction {
    fn txn(&self) -> &dyn Transaction;
    fn txn_mut(&mut self) -> &mut dyn Transaction;
    fn refresh(&mut self);
    fn renew(&mut self);
    fn commit(&mut self);
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
