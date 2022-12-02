use rsnano_core::{Account, BlockHash};

use crate::{DbIterator, ReadTransaction, Transaction, WriteTransaction};

pub type FrontierIterator = Box<dyn DbIterator<BlockHash, Account>>;

/// Maps head block to owning account
/// BlockHash -> Account
pub trait FrontierStore {
    fn put(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash, account: &Account);
    fn get(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<Account>;
    fn del(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash);
    fn begin(&self, txn: &dyn Transaction) -> FrontierIterator;

    fn begin_at_hash(&self, txn: &dyn Transaction, hash: &BlockHash) -> FrontierIterator;

    fn for_each_par(
        &self,
        action: &(dyn Fn(&dyn ReadTransaction, FrontierIterator, FrontierIterator) + Send + Sync),
    );

    fn end(&self) -> FrontierIterator;
}
