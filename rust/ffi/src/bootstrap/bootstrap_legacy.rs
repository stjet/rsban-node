use std::{
    ffi::{c_char, c_void, CStr},
    sync::Arc,
};

use crate::{block_processing::BlockProcessorHandle, ledger::datastore::LedgerHandle};

use super::{
    bootstrap_attempt::BootstrapAttemptHandle, pulls_cache::PullInfoDto, BootstrapInitiatorHandle,
};
use rsnano_core::BlockHash;
use rsnano_node::{
    bootstrap::{
        BootstrapAttemptLegacy, BootstrapStrategy, ADD_BULK_PUSH_TARGET, ADD_FRONTIER,
        ADD_START_ACCOUNT, REQUEST_BULK_PUSH_TARGET,
    },
    websocket::{Listener, NullListener, WebsocketListener},
};

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_legacy_create(
    cpp_handle: *mut c_void,
    websocket_server: *mut c_void,
    block_processor: &BlockProcessorHandle,
    bootstrap_initiator: &BootstrapInitiatorHandle,
    ledger: &LedgerHandle,
    id: *const c_char,
    incremental_id: u64,
) -> *mut BootstrapAttemptHandle {
    let id_str = CStr::from_ptr(id).to_str().unwrap();
    let websocket_server: Arc<dyn Listener> = if websocket_server.is_null() {
        Arc::new(NullListener::new())
    } else {
        Arc::new(WebsocketListener::new(websocket_server))
    };
    BootstrapAttemptHandle::new(Arc::new(BootstrapStrategy::Legacy(
        BootstrapAttemptLegacy::new(
            cpp_handle,
            websocket_server,
            Arc::downgrade(block_processor),
            Arc::downgrade(bootstrap_initiator),
            Arc::clone(ledger),
            id_str,
            incremental_id,
        )
        .unwrap(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_attempt_legacy_add_frontier(
    callback: LegacyAddFrontierCallback,
) {
    FFI_ADD_FRONTIER = Some(callback);
    ADD_FRONTIER = Some(|cpp_handle, pull| {
        let dto = PullInfoDto::from(pull);
        FFI_ADD_FRONTIER.unwrap()(cpp_handle, &dto);
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_attempt_legacy_add_start_account(
    callback: LegacyAddStartAccountCallback,
) {
    FFI_ADD_START_ACCOUNT = Some(callback);
    ADD_START_ACCOUNT = Some(|cpp_handle, account| {
        FFI_ADD_START_ACCOUNT.unwrap()(cpp_handle, account.as_bytes().as_ptr());
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_attempt_legacy_add_bulk_push_target(
    callback: LegacyAddBulkPushTargetCallback,
) {
    FFI_ADD_BULK_PUSH_TARGET = Some(callback);
    ADD_BULK_PUSH_TARGET = Some(|cpp_handle, head, end| {
        FFI_ADD_BULK_PUSH_TARGET.unwrap()(
            cpp_handle,
            head.as_bytes().as_ptr(),
            end.as_bytes().as_ptr(),
        );
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_attempt_legacy_request_bulk_push_target(
    callback: LegacyRequestBulkPushTargetCallback,
) {
    FFI_REQUEST_BULK_PUSH_TARGET = Some(callback);
    REQUEST_BULK_PUSH_TARGET = Some(|cpp_handle| {
        let mut head_bytes = [0_u8; 32];
        let mut end_bytes = [0_u8; 32];
        let is_empty = FFI_REQUEST_BULK_PUSH_TARGET.unwrap()(
            cpp_handle,
            head_bytes.as_mut_ptr(),
            end_bytes.as_mut_ptr(),
        );
        if is_empty {
            None
        } else {
            Some((
                BlockHash::from_bytes(head_bytes),
                BlockHash::from_bytes(end_bytes),
            ))
        }
    })
}

pub type LegacyAddFrontierCallback = unsafe extern "C" fn(*mut c_void, *const PullInfoDto);
pub type LegacyAddStartAccountCallback = unsafe extern "C" fn(*mut c_void, *const u8);
pub type LegacyAddBulkPushTargetCallback = unsafe extern "C" fn(*mut c_void, *const u8, *const u8);
pub type LegacyRequestBulkPushTargetCallback =
    unsafe extern "C" fn(*mut c_void, *mut u8, *mut u8) -> bool;

pub static mut FFI_ADD_FRONTIER: Option<LegacyAddFrontierCallback> = None;
pub static mut FFI_ADD_START_ACCOUNT: Option<LegacyAddStartAccountCallback> = None;
pub static mut FFI_ADD_BULK_PUSH_TARGET: Option<LegacyAddBulkPushTargetCallback> = None;
pub static mut FFI_REQUEST_BULK_PUSH_TARGET: Option<LegacyRequestBulkPushTargetCallback> = None;
