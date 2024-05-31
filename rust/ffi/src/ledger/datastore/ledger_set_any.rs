use super::TransactionHandle;
use crate::core::AccountInfoHandle;
use rsnano_core::Account;
use rsnano_ledger::LedgerSetAny;

pub struct LedgerSetAnyHandle(pub LedgerSetAny<'static>);

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_any_destroy(handle: *mut LedgerSetAnyHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_any_get_account(
    handle: &LedgerSetAnyHandle,
    tx: &TransactionHandle,
    account: *const u8,
) -> *mut AccountInfoHandle {
    match handle
        .0
        .get_account(tx.as_txn(), &Account::from_ptr(account))
    {
        Some(info) => Box::into_raw(Box::new(AccountInfoHandle(info))),
        None => std::ptr::null_mut(),
    }
}
