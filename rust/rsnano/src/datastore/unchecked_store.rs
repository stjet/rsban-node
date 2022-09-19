use crate::{unchecked_info::UncheckedInfo, HashOrAccount};

use super::WriteTransaction;

/// Unchecked bootstrap blocks info.
/// BlockHash -> UncheckedInfo
pub trait UncheckedStore {
    fn clear(&self, txn: &dyn WriteTransaction);
    fn put(&self, txn: &dyn WriteTransaction, dependency: &HashOrAccount, info: &UncheckedInfo);
}
