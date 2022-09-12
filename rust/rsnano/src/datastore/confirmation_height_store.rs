use super::WriteTransaction;
use crate::{Account, ConfirmationHeightInfo};

pub trait ConfirmationHeightStore {
    fn put(&self, txn: &dyn WriteTransaction, account: &Account, info: &ConfirmationHeightInfo);
}
