use super::{DbIterator, Transaction, WriteTransaction};
use crate::{Account, AccountInfo};

pub trait AccountStore {
    fn put(&self, transaction: &dyn WriteTransaction, account: &Account, info: &AccountInfo);
    fn get(&self, transaction: &dyn Transaction, account: &Account) -> Option<AccountInfo>;
    fn del(&self, transaction: &dyn WriteTransaction, account: &Account);
    fn begin(
        &self,
        transaction: &dyn Transaction,
        account: &Account,
    ) -> Box<dyn DbIterator<Account, AccountInfo>>;
}
