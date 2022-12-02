use rsnano_core::{HashOrAccount, UncheckedInfo, UncheckedKey};

use crate::{DbIterator, Transaction, WriteTransaction};

pub type UncheckedIterator = Box<dyn DbIterator<UncheckedKey, UncheckedInfo>>;

/// Unchecked bootstrap blocks info.
/// BlockHash -> UncheckedInfo
pub trait UncheckedStore {
    fn clear(&self, txn: &mut dyn WriteTransaction);
    fn put(&self, txn: &mut dyn WriteTransaction, dependency: &HashOrAccount, info: &UncheckedInfo);
    fn exists(&self, txn: &dyn Transaction, key: &UncheckedKey) -> bool;
    fn del(&self, txn: &mut dyn WriteTransaction, key: &UncheckedKey);
    fn begin(&self, txn: &dyn Transaction) -> UncheckedIterator;
    fn lower_bound(&self, txn: &dyn Transaction, key: &UncheckedKey) -> UncheckedIterator;
    fn count(&self, txn: &dyn Transaction) -> u64;
}
