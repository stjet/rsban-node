use super::{iterator::DbIteratorImpl, DbIterator, Transaction, WriteTransaction};
use crate::core::{HashOrAccount, UncheckedInfo, UncheckedKey};

pub type UncheckedIterator<I> = DbIterator<UncheckedKey, UncheckedInfo, I>;

/// Unchecked bootstrap blocks info.
/// BlockHash -> UncheckedInfo
pub trait UncheckedStore<I>
where
    I: DbIteratorImpl,
{
    fn clear(&self, txn: &mut dyn WriteTransaction);
    fn put(&self, txn: &mut dyn WriteTransaction, dependency: &HashOrAccount, info: &UncheckedInfo);
    fn exists(&self, txn: &dyn Transaction, key: &UncheckedKey) -> bool;
    fn del(&self, txn: &mut dyn WriteTransaction, key: &UncheckedKey);
    fn begin(&self, txn: &dyn Transaction) -> UncheckedIterator<I>;
    fn lower_bound(&self, txn: &dyn Transaction, key: &UncheckedKey) -> UncheckedIterator<I>;
    fn count(&self, txn: &dyn Transaction) -> usize;
}
