use super::{iterator::DbIteratorImpl, DbIterator, Transaction};
use crate::core::{Account, AccountInfo};

pub type AccountIterator<I> = DbIterator<Account, AccountInfo, I>;

pub trait AccountStore<'a, R, W, I>
where
    R: 'a,
    W: 'a,
    I: DbIteratorImpl,
{
    fn put(&self, transaction: &mut W, account: &Account, info: &AccountInfo);
    fn get(&self, transaction: &Transaction<R, W>, account: &Account) -> Option<AccountInfo>;
    fn del(&self, transaction: &mut W, account: &Account);
    fn begin_account(
        &self,
        transaction: &Transaction<R, W>,
        account: &Account,
    ) -> DbIterator<Account, AccountInfo, I>;
    fn begin(&self, transaction: &Transaction<R, W>) -> AccountIterator<I>;
    fn for_each_par(
        &'a self,
        action: &(dyn Fn(R, AccountIterator<I>, AccountIterator<I>) + Send + Sync),
    );
    fn end(&self) -> AccountIterator<I>;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn exists(&self, txn: &Transaction<R, W>, account: &Account) -> bool;
}
