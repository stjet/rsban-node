use crate::core::Amount;

use super::{iterator::DbIteratorImpl, DbIterator, Transaction, WriteTransaction};

pub type OnlineWeightIterator<I> = DbIterator<u64, Amount, I>;

/// Samples of online vote weight
/// u64 -> Amount
pub trait OnlineWeightStore<I>
where
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut dyn WriteTransaction, time: u64, amount: &Amount);
    fn del(&self, txn: &mut dyn WriteTransaction, time: u64);
    fn begin(&self, txn: &dyn Transaction) -> OnlineWeightIterator<I>;
    fn rbegin(&self, txn: &dyn Transaction) -> OnlineWeightIterator<I>;
    fn count(&self, txn: &dyn Transaction) -> usize;
    fn clear(&self, txn: &mut dyn WriteTransaction);
}
