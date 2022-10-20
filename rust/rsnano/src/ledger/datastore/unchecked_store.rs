use crate::core::{HashOrAccount, UncheckedInfo, UncheckedKey};

use super::{iterator::DbIteratorImpl, DbIterator, Transaction};

pub type UncheckedIterator<I> = DbIterator<UncheckedKey, UncheckedInfo, I>;

/// Unchecked bootstrap blocks info.
/// BlockHash -> UncheckedInfo
pub trait UncheckedStore<'a, R, W, I>
where
    R: 'a,
    W: 'a,
    I: DbIteratorImpl,
{
    fn clear(&self, txn: &mut W);
    fn put(&self, txn: &mut W, dependency: &HashOrAccount, info: &UncheckedInfo);
    fn exists(&self, txn: &Transaction<R, W>, key: &UncheckedKey) -> bool;
    fn del(&self, txn: &mut W, key: &UncheckedKey);
    fn begin(&self, txn: &Transaction<R, W>) -> UncheckedIterator<I>;
    fn lower_bound(&self, txn: &Transaction<R, W>, key: &UncheckedKey) -> UncheckedIterator<I>;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
}
