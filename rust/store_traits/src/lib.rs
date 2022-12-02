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
