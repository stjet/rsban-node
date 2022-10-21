use super::{iterator::DbIteratorImpl, DbIterator, ReadTransaction, Transaction, WriteTransaction};
use crate::core::{Account, ConfirmationHeightInfo};

pub type ConfirmationHeightIterator<I> = DbIterator<Account, ConfirmationHeightInfo, I>;

pub trait ConfirmationHeightStore<I>
where
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut dyn WriteTransaction, account: &Account, info: &ConfirmationHeightInfo);
    fn get(&self, txn: &dyn Transaction, account: &Account) -> Option<ConfirmationHeightInfo>;
    fn exists(&self, txn: &dyn Transaction, account: &Account) -> bool;
    fn del(&self, txn: &mut dyn WriteTransaction, account: &Account);
    fn count(&self, txn: &dyn Transaction) -> usize;
    fn clear(&self, txn: &mut dyn WriteTransaction);
    fn begin(&self, txn: &dyn Transaction) -> ConfirmationHeightIterator<I>;
    fn begin_at_account(
        &self,
        txn: &dyn Transaction,
        account: &Account,
    ) -> ConfirmationHeightIterator<I>;
    fn end(&self) -> ConfirmationHeightIterator<I>;
    fn for_each_par(
        &self,
        action: &(dyn Fn(&dyn ReadTransaction, ConfirmationHeightIterator<I>, ConfirmationHeightIterator<I>)
              + Send
              + Sync),
    );
}
