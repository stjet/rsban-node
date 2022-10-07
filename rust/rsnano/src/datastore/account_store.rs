use super::{iterator::DbIteratorImpl, DbIterator2, Transaction};
use crate::{Account, AccountInfo};

pub type AccountIterator<I> = DbIterator2<Account, AccountInfo, I>;

pub trait AccountStore<R, W, I>
where
    I: DbIteratorImpl,
{
    fn put(&self, transaction: &W, account: &Account, info: &AccountInfo);
    fn get(&self, transaction: &Transaction<R, W>, account: &Account) -> Option<AccountInfo>;
    fn del(&self, transaction: &W, account: &Account);
    fn begin_account(
        &self,
        transaction: &Transaction<R, W>,
        account: &Account,
    ) -> DbIterator2<Account, AccountInfo, I>;
    fn begin(&self, transaction: &Transaction<R, W>) -> AccountIterator<I>;
    fn for_each_par(
        &self,
        action: &(dyn Fn(&R, AccountIterator<I>, AccountIterator<I>) + Send + Sync),
    );
    fn end(&self) -> AccountIterator<I>;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
}
