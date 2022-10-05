use crate::Amount;

use super::{DbIterator, Transaction};

/// Samples of online vote weight
/// u64 -> Amount
pub trait OnlineWeightStore<R, W> {
    fn put(&self, txn: &W, time: u64, amount: &Amount);
    fn del(&self, txn: &W, time: u64);
    fn begin(&self, txn: &Transaction<R, W>) -> Box<dyn DbIterator<u64, Amount>>;
    fn rbegin(&self, txn: &Transaction<R, W>) -> Box<dyn DbIterator<u64, Amount>>;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn clear(&self, txn: &W);
}
