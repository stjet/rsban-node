use super::{iterator::DbIteratorImpl, DbIterator, Transaction};
use crate::{Account, BlockHash};

pub type FrontierIterator<I> = DbIterator<BlockHash, Account, I>;

/// Maps head block to owning account
/// BlockHash -> Account
pub trait FrontierStore<'a, R, W, I>
where
    R: 'a,
    W: 'a,
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut W, hash: &BlockHash, account: &Account);
    fn get(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Account;
    fn del(&self, txn: &mut W, hash: &BlockHash);
    fn begin(&self, txn: &Transaction<R, W>) -> FrontierIterator<I>;

    fn begin_at_hash(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> FrontierIterator<I>;

    fn for_each_par(
        &'a self,
        action: &(dyn Fn(R, FrontierIterator<I>, FrontierIterator<I>) + Send + Sync),
    );

    fn end(&self) -> FrontierIterator<I>;
}
