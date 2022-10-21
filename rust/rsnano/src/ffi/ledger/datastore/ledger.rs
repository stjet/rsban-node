use crate::ledger::Ledger;
use std::{ffi::c_void, ops::Deref, sync::Arc};

use super::lmdb::LmdbStoreHandle;

pub struct LedgerHandle(Arc<Ledger>);

impl Deref for LedgerHandle {
    type Target = Arc<Ledger>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_ledger_create(
    handle: *mut c_void,
    _store: *mut LmdbStoreHandle,
) -> *mut LedgerHandle {
    let ledger = Ledger::new(handle);
    Box::into_raw(Box::new(LedgerHandle(Arc::new(ledger))))
}

#[no_mangle]
pub extern "C" fn rsn_ledger_destroy(handle: *mut LedgerHandle) {
    drop(unsafe { Box::from_raw(handle) });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_ledger_block_or_pruned_exists(
    f: LedgerBlockOrPrunedExistsCallback,
) {
    BLOCK_OR_PRUNED_EXISTS_CALLBACK = Some(f);
}

type LedgerBlockOrPrunedExistsCallback = unsafe extern "C" fn(*mut c_void, *const u8) -> bool;
pub(crate) static mut BLOCK_OR_PRUNED_EXISTS_CALLBACK: Option<LedgerBlockOrPrunedExistsCallback> =
    None;

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_pruning_enabled(handle: *mut LedgerHandle) -> bool {
    (*handle).0.pruning_enabled()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_enable_pruning(handle: *mut LedgerHandle) {
    (*handle).0.enable_pruning()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_bootstrap_weight_max_blocks(handle: *mut LedgerHandle) -> u64 {
    (*handle).0.bootstrap_weight_max_blocks()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_bootstrap_weight_max_blocks(
    handle: *mut LedgerHandle,
    max: u64,
) {
    (*handle).0.set_bootstrap_weight_max_blocks(max)
}
