use super::{DbIterator, Transaction, WriteTransaction};
use crate::{BlockHash, QualifiedRoot, Root};

pub trait FinalVoteStore {
    fn put(&self, txn: &dyn WriteTransaction, root: &QualifiedRoot, hash: &BlockHash) -> bool;
    fn begin(&self, txn: &dyn Transaction) -> Box<dyn DbIterator<QualifiedRoot, BlockHash>>;
    fn begin_at_root(
        &self,
        txn: &dyn Transaction,
        root: &QualifiedRoot,
    ) -> Box<dyn DbIterator<QualifiedRoot, BlockHash>>;
    fn get(&self, txn: &dyn Transaction, root: Root) -> Vec<BlockHash>;
}
