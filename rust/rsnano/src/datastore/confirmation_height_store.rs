use super::{DbIterator, Transaction};
use crate::{Account, ConfirmationHeightInfo};

pub trait ConfirmationHeightStore<R, W> {
    fn put(&self, txn: &W, account: &Account, info: &ConfirmationHeightInfo);
    fn get(&self, txn: &Transaction<R, W>, account: &Account) -> Option<ConfirmationHeightInfo>;
    fn exists(&self, txn: &Transaction<R, W>, account: &Account) -> bool;
    fn del(&self, txn: &Transaction<R, W>, account: &Account);
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn clear(&self, txn: &W);
    fn begin(
        &self,
        txn: &Transaction<R, W>,
    ) -> Box<dyn DbIterator<Account, ConfirmationHeightInfo>>;
    fn begin_at_account(
        &self,
        txn: &Transaction<R, W>,
        account: &Account,
    ) -> Box<dyn DbIterator<Account, ConfirmationHeightInfo>>;
    fn end(&self) -> Box<dyn DbIterator<Account, ConfirmationHeightInfo>>;
    fn for_each_par(
        &self,
        action: &(dyn Fn(
            &R,
            &mut dyn DbIterator<Account, ConfirmationHeightInfo>,
            &mut dyn DbIterator<Account, ConfirmationHeightInfo>,
        ) + Send
              + Sync),
    );
}
