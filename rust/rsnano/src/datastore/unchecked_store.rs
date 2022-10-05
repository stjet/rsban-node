use crate::{
    unchecked_info::{UncheckedInfo, UncheckedKey},
    HashOrAccount,
};

use super::{DbIterator, Transaction};

/// Unchecked bootstrap blocks info.
/// BlockHash -> UncheckedInfo
pub trait UncheckedStore<R, W> {
    fn clear(&self, txn: &W);
    fn put(&self, txn: &W, dependency: &HashOrAccount, info: &UncheckedInfo);
    fn exists(&self, txn: &Transaction<R, W>, key: &UncheckedKey) -> bool;
    fn del(&self, txn: &W, key: &UncheckedKey);
    fn begin(&self, txn: &Transaction<R, W>) -> Box<dyn DbIterator<UncheckedKey, UncheckedInfo>>;
    fn lower_bound(
        &self,
        txn: &Transaction<R, W>,
        key: &UncheckedKey,
    ) -> Box<dyn DbIterator<UncheckedKey, UncheckedInfo>>;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
}
