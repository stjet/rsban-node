use super::{DbIterator, Transaction};
use crate::{Account, AccountInfo};

pub trait AccountStore<R, W, IT>
where
    IT: DbIterator<Account, AccountInfo>,
{
    fn put(&self, transaction: &W, account: &Account, info: &AccountInfo);
    fn get(&self, transaction: &Transaction<R, W>, account: &Account) -> Option<AccountInfo>;
    fn del(&self, transaction: &W, account: &Account);
    fn begin_account(&self, transaction: &Transaction<R, W>, account: &Account) -> IT;
    fn begin(&self, transaction: &Transaction<R, W>) -> IT;
    fn rbegin(&self, transaction: &Transaction<R, W>) -> IT;
    fn for_each_par(&self, action: &(dyn Fn(&R, &mut IT, &mut IT) + Send + Sync));
    fn end(&self) -> IT;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
}
