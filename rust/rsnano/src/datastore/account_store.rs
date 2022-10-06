use super::{DbIterator, Transaction};
use crate::{Account, AccountInfo};

pub trait AccountStore<R, W> {
    fn put(&self, transaction: &W, account: &Account, info: &AccountInfo);
    fn get(&self, transaction: &Transaction<R, W>, account: &Account) -> Option<AccountInfo>;
    fn del(&self, transaction: &W, account: &Account);
    fn begin_account(
        &self,
        transaction: &Transaction<R, W>,
        account: &Account,
    ) -> Box<dyn DbIterator<Account, AccountInfo>>;
    fn begin(&self, transaction: &Transaction<R, W>) -> Box<dyn DbIterator<Account, AccountInfo>>;
    fn rbegin(&self, transaction: &Transaction<R, W>) -> Box<dyn DbIterator<Account, AccountInfo>>;
    fn for_each_par(
        &self,
        action: &(dyn Fn(
            &R,
            &mut dyn DbIterator<Account, AccountInfo>,
            &mut dyn DbIterator<Account, AccountInfo>,
        ) + Send
              + Sync),
    );
    fn end(&self) -> Box<dyn DbIterator<Account, AccountInfo>>;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
}
