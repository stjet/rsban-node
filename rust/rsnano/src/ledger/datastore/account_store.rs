use super::{iterator::DbIteratorImpl, DbIterator, ReadTransaction, Transaction, WriteTransaction};
use crate::core::{Account, AccountInfo};

pub type AccountIterator<I> = DbIterator<Account, AccountInfo, I>;

pub trait AccountStore<I>
where
    I: DbIteratorImpl,
{
    fn put(&self, transaction: &mut dyn WriteTransaction, account: &Account, info: &AccountInfo);
    fn get(&self, transaction: &dyn Transaction, account: &Account) -> Option<AccountInfo>;
    fn del(&self, transaction: &mut dyn WriteTransaction, account: &Account);
    fn begin_account(
        &self,
        transaction: &dyn Transaction,
        account: &Account,
    ) -> DbIterator<Account, AccountInfo, I>;
    fn begin(&self, transaction: &dyn Transaction) -> AccountIterator<I>;
    fn for_each_par(
        &self,
        action: &(dyn Fn(&dyn ReadTransaction, AccountIterator<I>, AccountIterator<I>)
              + Send
              + Sync),
    );
    fn end(&self) -> AccountIterator<I>;
    fn count(&self, txn: &dyn Transaction) -> usize;
    fn exists(&self, txn: &dyn Transaction, account: &Account) -> bool;
}
