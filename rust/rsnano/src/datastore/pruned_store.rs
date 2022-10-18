use super::{iterator::DbIteratorImpl, DbIterator, Transaction};
use crate::core::{BlockHash, NoValue};

pub type PrunedIterator<I> = DbIterator<BlockHash, NoValue, I>;

/// Pruned blocks hashes
pub trait PrunedStore<'a, R, W, I>
where
    R: 'a,
    W: 'a,
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut W, hash: &BlockHash);
    fn del(&self, txn: &mut W, hash: &BlockHash);
    fn exists(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> bool;
    fn begin(&self, txn: &Transaction<R, W>) -> PrunedIterator<I>;

    fn begin_at_hash(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> PrunedIterator<I>;

    fn end(&self) -> PrunedIterator<I>;

    fn random(&self, txn: &Transaction<R, W>) -> BlockHash;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn clear(&self, txn: &mut W);
    fn for_each_par(
        &'a self,
        action: &(dyn Fn(R, PrunedIterator<I>, PrunedIterator<I>) + Send + Sync),
    );
}
