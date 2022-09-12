use super::{DbIterator, Transaction, WriteTransaction};
use crate::{BlockHash, QualifiedRoot};

pub trait FinalVoteStore {
    fn put(&self, txn: &dyn WriteTransaction, root: &QualifiedRoot, hash: &BlockHash) -> bool;
    fn begin(&self, txn: &dyn Transaction) -> Box<dyn DbIterator<QualifiedRoot, BlockHash>>;
}
