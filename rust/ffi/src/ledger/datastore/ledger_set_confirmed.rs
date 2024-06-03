use super::TransactionHandle;
use rsnano_core::{Account, BlockHash};
use rsnano_ledger::LedgerSetConfirmed;

pub struct LedgerSetConfirmedHandle(pub LedgerSetConfirmed<'static>);

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_confirmed_destroy(handle: *mut LedgerSetConfirmedHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_confirmed_block_exists_or_pruned(
    handle: &LedgerSetConfirmedHandle,
    tx: &TransactionHandle,
    hash: *const u8,
) -> bool {
    handle
        .0
        .block_exists_or_pruned(tx.as_txn(), &BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_confirmed_block_exists(
    handle: &LedgerSetConfirmedHandle,
    tx: &TransactionHandle,
    hash: *const u8,
) -> bool {
    handle
        .0
        .block_exists(tx.as_txn(), &BlockHash::from_ptr(hash))
}
