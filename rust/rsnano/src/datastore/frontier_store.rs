use crate::{Account, BlockHash};

use super::{Transaction, WriteTransaction};

/// Maps head block to owning account
/// BlockHash -> Account
pub trait FrontierStore {
    fn put(&self, txn: &dyn WriteTransaction, hash: &BlockHash, account: &Account);
    fn get(&self, txn: &dyn Transaction, hash: &BlockHash) -> Account;
}
