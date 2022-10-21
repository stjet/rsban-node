use super::{iterator::DbIteratorImpl, DbIterator, ReadTransaction, Transaction, WriteTransaction};
use crate::core::{Account, BlockHash};

pub type FrontierIterator<I> = DbIterator<BlockHash, Account, I>;

/// Maps head block to owning account
/// BlockHash -> Account
pub trait FrontierStore<I>
where
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash, account: &Account);
    fn get(&self, txn: &dyn Transaction, hash: &BlockHash) -> Account;
    fn del(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash);
    fn begin(&self, txn: &dyn Transaction) -> FrontierIterator<I>;

    fn begin_at_hash(&self, txn: &dyn Transaction, hash: &BlockHash) -> FrontierIterator<I>;

    fn for_each_par(
        &self,
        action: &(dyn Fn(&dyn ReadTransaction, FrontierIterator<I>, FrontierIterator<I>)
              + Send
              + Sync),
    );

    fn end(&self) -> FrontierIterator<I>;
}
