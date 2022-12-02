use rsnano_core::{BlockHash, NoValue};

use crate::{DbIterator, ReadTransaction, Transaction, WriteTransaction};

pub type PrunedIterator = Box<dyn DbIterator<BlockHash, NoValue>>;

/// Pruned blocks hashes
pub trait PrunedStore {
    fn put(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash);
    fn del(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash);
    fn exists(&self, txn: &dyn Transaction, hash: &BlockHash) -> bool;
    fn begin(&self, txn: &dyn Transaction) -> PrunedIterator;

    fn begin_at_hash(&self, txn: &dyn Transaction, hash: &BlockHash) -> PrunedIterator;

    fn end(&self) -> PrunedIterator;

    fn random(&self, txn: &dyn Transaction) -> Option<BlockHash>;
    fn count(&self, txn: &dyn Transaction) -> u64;
    fn clear(&self, txn: &mut dyn WriteTransaction);
    fn for_each_par(
        &self,
        action: &(dyn Fn(&dyn ReadTransaction, PrunedIterator, PrunedIterator) + Send + Sync),
    );
}
