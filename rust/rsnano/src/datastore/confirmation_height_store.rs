use super::{Transaction, WriteTransaction};
use crate::{Account, ConfirmationHeightInfo};

pub trait ConfirmationHeightStore {
    fn put(&self, txn: &dyn WriteTransaction, account: &Account, info: &ConfirmationHeightInfo);
    fn get(&self, txn: &dyn Transaction, account: &Account) -> Option<ConfirmationHeightInfo>;
    fn exists(&self, txn: &dyn Transaction, account: &Account) -> bool;
}
