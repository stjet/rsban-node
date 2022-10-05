use super::{DbIterator, Transaction};
use crate::{BlockHash, NoValue};

/// Pruned blocks hashes
pub trait PrunedStore<R, W> {
    fn put(&self, txn: &W, hash: &BlockHash);
    fn del(&self, txn: &W, hash: &BlockHash);
    fn exists(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> bool;
    fn begin(&self, txn: &Transaction<R, W>) -> Box<dyn DbIterator<BlockHash, NoValue>>;

    fn begin_at_hash(
        &self,
        txn: &Transaction<R, W>,
        hash: &BlockHash,
    ) -> Box<dyn DbIterator<BlockHash, NoValue>>;

    fn end(&self) -> Box<dyn DbIterator<BlockHash, NoValue>>;

    fn random(&self, txn: &Transaction<R, W>) -> BlockHash;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn clear(&self, txn: &W);
    fn for_each_par(
        &self,
        action: &(dyn Fn(
            &R,
            &mut dyn DbIterator<BlockHash, NoValue>,
            &mut dyn DbIterator<BlockHash, NoValue>,
        ) + Send
              + Sync),
    );
}
