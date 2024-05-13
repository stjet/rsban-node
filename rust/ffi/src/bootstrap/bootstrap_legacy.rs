use super::{
    bootstrap_attempt::BootstrapAttemptHandle, bootstrap_connections::BootstrapConnectionsHandle,
    pulls_cache::PullInfoDto, BootstrapInitiatorHandle,
};
use crate::{
    block_processing::BlockProcessorHandle, ledger::datastore::LedgerHandle,
    websocket::WebsocketListenerHandle, FfiPropertyTree, NetworkParamsDto, NodeConfigDto,
    NodeFlagsHandle, StatHandle,
};
use rsnano_core::{Account, BlockHash};
use rsnano_node::bootstrap::{BootstrapAttemptLegacy, BootstrapStrategy};
use std::{
    ffi::{c_char, c_void, CStr},
    ops::Deref,
    sync::Arc,
};

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_legacy_create(
    websocket_server: *mut WebsocketListenerHandle,
    block_processor: &BlockProcessorHandle,
    bootstrap_initiator: &BootstrapInitiatorHandle,
    ledger: &LedgerHandle,
    id: *const c_char,
    incremental_id: u64,
    bootstrap_connections: &BootstrapConnectionsHandle,
    network_params: &NetworkParamsDto,
    config: &NodeConfigDto,
    flags: &NodeFlagsHandle,
    stats: &StatHandle,
    frontiers_age: u32,
    start_account: *const u8,
) -> *mut BootstrapAttemptHandle {
    let id_str = CStr::from_ptr(id).to_str().unwrap();
    let websocket_server = if websocket_server.is_null() {
        None
    } else {
        Some(Arc::clone((*websocket_server).deref()))
    };
    BootstrapAttemptHandle::new(Arc::new(BootstrapStrategy::Legacy(Arc::new(
        BootstrapAttemptLegacy::new(
            websocket_server,
            Arc::downgrade(block_processor),
            Arc::downgrade(bootstrap_initiator),
            Arc::clone(ledger),
            id_str,
            incremental_id,
            Arc::clone(bootstrap_connections),
            network_params.try_into().unwrap(),
            config.try_into().unwrap(),
            Arc::clone(stats),
            flags.lock().unwrap().clone(),
            frontiers_age,
            Account::from_ptr(start_account),
        )
        .unwrap(),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_legacy_add_frontier(
    handle: &BootstrapAttemptHandle,
    pull_info: &PullInfoDto,
) {
    let BootstrapStrategy::Legacy(legacy) = &***handle else {
        panic!("not legacy");
    };
    legacy.add_frontier(pull_info.into());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_legacy_add_bulk_push_target(
    handle: &BootstrapAttemptHandle,
    head: *const u8,
    end: *const u8,
) {
    let BootstrapStrategy::Legacy(legacy) = &***handle else {
        panic!("not legacy");
    };
    legacy.add_bulk_push_target(BlockHash::from_ptr(head), BlockHash::from_ptr(end));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_legacy_request_bulk_push_target(
    handle: &BootstrapAttemptHandle,
    head: *mut u8,
    end: *mut u8,
) -> bool {
    let BootstrapStrategy::Legacy(legacy) = &***handle else {
        panic!("not legacy");
    };
    match legacy.request_bulk_push_target() {
        Some((h, e)) => {
            h.copy_bytes(head);
            e.copy_bytes(end);
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_legacy_set_start_account(
    handle: &BootstrapAttemptHandle,
    account: *const u8,
) {
    let BootstrapStrategy::Legacy(legacy) = &***handle else {
        panic!("not legacy");
    };
    legacy.set_start_account(Account::from_ptr(account));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_legacy_get_information(
    handle: &BootstrapAttemptHandle,
    ptree: *mut c_void,
) {
    let BootstrapStrategy::Legacy(legacy) = &***handle else {
        panic!("not legacy");
    };
    let mut tree = FfiPropertyTree::new_borrowed(ptree);
    legacy.get_information(&mut tree);
}
