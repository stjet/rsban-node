use super::{iterator::DbIteratorImpl, DbIterator, Transaction};
use crate::core::{Account, ConfirmationHeightInfo};

pub type ConfirmationHeightIterator<I> = DbIterator<Account, ConfirmationHeightInfo, I>;

pub trait ConfirmationHeightStore<'a, R, W, I>
where
    R: 'a,
    W: 'a,
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut W, account: &Account, info: &ConfirmationHeightInfo);
    fn get(&self, txn: &Transaction<R, W>, account: &Account) -> Option<ConfirmationHeightInfo>;
    fn exists(&self, txn: &Transaction<R, W>, account: &Account) -> bool;
    fn del(&self, txn: &mut W, account: &Account);
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn clear(&self, txn: &mut W);
    fn begin(&self, txn: &Transaction<R, W>) -> ConfirmationHeightIterator<I>;
    fn begin_at_account(
        &self,
        txn: &Transaction<R, W>,
        account: &Account,
    ) -> ConfirmationHeightIterator<I>;
    fn end(&self) -> ConfirmationHeightIterator<I>;
    fn for_each_par(
        &'a self,
        action: &(dyn Fn(R, ConfirmationHeightIterator<I>, ConfirmationHeightIterator<I>)
              + Send
              + Sync),
    );
}
