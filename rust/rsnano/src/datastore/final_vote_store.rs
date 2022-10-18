use super::{iterator::DbIteratorImpl, DbIterator, Transaction};
use crate::core::{BlockHash, QualifiedRoot, Root};

pub type FinalVoteIterator<I> = DbIterator<QualifiedRoot, BlockHash, I>;

pub trait FinalVoteStore<'a, R, W, I>
where
    R: 'a,
    W: 'a,
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut W, root: &QualifiedRoot, hash: &BlockHash) -> bool;
    fn begin(&self, txn: &Transaction<R, W>) -> FinalVoteIterator<I>;
    fn begin_at_root(&self, txn: &Transaction<R, W>, root: &QualifiedRoot) -> FinalVoteIterator<I>;
    fn end(&self) -> FinalVoteIterator<I>;
    fn get(&self, txn: &Transaction<R, W>, root: Root) -> Vec<BlockHash>;
    fn del(&self, txn: &mut W, root: &Root);
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn clear(&self, txn: &mut W);
    fn for_each_par(
        &'a self,
        action: &(dyn Fn(R, FinalVoteIterator<I>, FinalVoteIterator<I>) + Send + Sync),
    );
}
