use std::sync::Arc;

use super::TransactionHandle;
use crate::core::{AccountInfoHandle, BlockHandle};
use rsnano_core::{Account, BlockHash};
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

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_any_block_exists(
    handle: &LedgerSetAnyHandle,
    tx: &TransactionHandle,
    hash: *const u8,
) -> bool {
    handle
        .0
        .block_exists(tx.as_txn(), &BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_any_block_exists_or_pruned(
    handle: &LedgerSetAnyHandle,
    tx: &TransactionHandle,
    hash: *const u8,
) -> bool {
    handle
        .0
        .block_exists_or_pruned(tx.as_txn(), &BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_any_block_get(
    handle: &LedgerSetAnyHandle,
    tx: &TransactionHandle,
    hash: *const u8,
) -> *mut BlockHandle {
    match handle.0.get_block(tx.as_txn(), &BlockHash::from_ptr(hash)) {
        Some(block) => BlockHandle::new(Arc::new(block)),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_any_block_balance(
    handle: &LedgerSetAnyHandle,
    tx: &TransactionHandle,
    hash: *const u8,
    amount: *mut u8,
) -> bool {
    match handle
        .0
        .block_balance(tx.as_txn(), &BlockHash::from_ptr(hash))
    {
        Some(balance) => {
            balance.copy_bytes(amount);
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_any_account_head(
    handle: &LedgerSetAnyHandle,
    tx: &TransactionHandle,
    account: *const u8,
    head: *mut u8,
) -> bool {
    match handle
        .0
        .account_head(tx.as_txn(), &Account::from_ptr(account))
    {
        Some(hash) => {
            hash.copy_bytes(head);
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_any_block_account(
    handle: &LedgerSetAnyHandle,
    tx: &TransactionHandle,
    hash: *const u8,
    account: *mut u8,
) -> bool {
    match handle
        .0
        .block_account(tx.as_txn(), &BlockHash::from_ptr(account))
    {
        Some(acc) => {
            acc.copy_bytes(account);
            true
        }
        None => false,
    }
}
