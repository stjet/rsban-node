use super::{DbIterator, Transaction};
use crate::{BlockHash, QualifiedRoot, Root};

pub trait FinalVoteStore<R, W> {
    fn put(&self, txn: &W, root: &QualifiedRoot, hash: &BlockHash) -> bool;
    fn begin(&self, txn: &Transaction<R, W>) -> Box<dyn DbIterator<QualifiedRoot, BlockHash>>;
    fn begin_at_root(
        &self,
        txn: &Transaction<R, W>,
        root: &QualifiedRoot,
    ) -> Box<dyn DbIterator<QualifiedRoot, BlockHash>>;
    fn end(&self) -> Box<dyn DbIterator<QualifiedRoot, BlockHash>>;
    fn get(&self, txn: &Transaction<R, W>, root: Root) -> Vec<BlockHash>;
    fn del(&self, txn: &W, root: Root);
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn clear(&self, txn: &W);
    fn for_each_par(
        &self,
        action: &(dyn Fn(
            R,
            &mut dyn DbIterator<QualifiedRoot, BlockHash>,
            &mut dyn DbIterator<QualifiedRoot, BlockHash>,
        ) + Send
              + Sync),
    );
}
