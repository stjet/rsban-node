use crate::core::Amount;

use super::{iterator::DbIteratorImpl, DbIterator, Transaction};

pub type OnlineWeightIterator<I> = DbIterator<u64, Amount, I>;

/// Samples of online vote weight
/// u64 -> Amount
pub trait OnlineWeightStore<'a, R, W, I>
where
    R: 'a,
    W: 'a,
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut W, time: u64, amount: &Amount);
    fn del(&self, txn: &mut W, time: u64);
    fn begin(&self, txn: &Transaction<R, W>) -> OnlineWeightIterator<I>;
    fn rbegin(&self, txn: &Transaction<R, W>) -> OnlineWeightIterator<I>;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn clear(&self, txn: &mut W);
}
