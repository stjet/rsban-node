use super::{iterator::DbIteratorImpl, DbIterator, ReadTransaction, Transaction, WriteTransaction};
use crate::core::{BlockHash, NoValue};

pub type PrunedIterator<I> = DbIterator<BlockHash, NoValue, I>;

/// Pruned blocks hashes
pub trait PrunedStore<I>
where
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash);
    fn del(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash);
    fn exists(&self, txn: &dyn Transaction, hash: &BlockHash) -> bool;
    fn begin(&self, txn: &dyn Transaction) -> PrunedIterator<I>;

    fn begin_at_hash(&self, txn: &dyn Transaction, hash: &BlockHash) -> PrunedIterator<I>;

    fn end(&self) -> PrunedIterator<I>;

    fn random(&self, txn: &dyn Transaction) -> BlockHash;
    fn count(&self, txn: &dyn Transaction) -> usize;
    fn clear(&self, txn: &mut dyn WriteTransaction);
    fn for_each_par(
        &self,
        action: &(dyn Fn(&dyn ReadTransaction, PrunedIterator<I>, PrunedIterator<I>) + Send + Sync),
    );
}
