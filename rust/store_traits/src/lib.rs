mod iterator;
pub use iterator::{BinaryDbIterator, DbIterator, DbIteratorImpl};

mod account_store;
pub use account_store::{AccountIterator, AccountStore};

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

mod unchecked_store;
pub use unchecked_store::{UncheckedIterator, UncheckedStore};

mod version_store;
pub use version_store::VersionStore;

mod store;
pub use store::Store;

use std::any::Any;

pub trait Transaction {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait ReadTransaction {
    fn txn(&self) -> &dyn Transaction;
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

pub trait TxnCallbacks {
    fn txn_start(&self, txn_id: u64, is_write: bool);
    fn txn_end(&self, txn_id: u64, is_write: bool);
}

pub struct NullTxnCallbacks {}

impl NullTxnCallbacks {
    pub fn new() -> Self {
        Self {}
    }
}

impl TxnCallbacks for NullTxnCallbacks {
    fn txn_start(&self, _txn_id: u64, _is_write: bool) {}

    fn txn_end(&self, _txn_id: u64, _is_write: bool) {}
}
