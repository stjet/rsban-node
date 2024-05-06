use super::{
    bootstrap_attempt::BootstrapAttemptHandle, bootstrap_connections::BootstrapConnectionsHandle,
    bootstrap_initiator::BootstrapInitiatorHandle, pulls_cache::PullInfoDto,
};
use crate::{
    block_processing::BlockProcessorHandle, ledger::datastore::LedgerHandle,
    websocket::WebsocketListenerHandle, FfiPropertyTree, NetworkParamsDto, NodeFlagsHandle,
};
use rsnano_core::{BlockHash, HashOrAccount};
use rsnano_node::{
    bootstrap::{BootstrapAttemptLazy, BootstrapStrategy},
    websocket::{Listener, NullListener},
    NetworkParams,
};
use std::{
    ffi::{c_void, CStr},
    ops::Deref,
    os::raw::c_char,
    sync::Arc,
};

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_lazy_create(
    self_ptr: *mut c_void,
    websocket_server: *mut WebsocketListenerHandle,
    block_processor: &BlockProcessorHandle,
    bootstrap_initiator: *const BootstrapInitiatorHandle,
    ledger: *const LedgerHandle,
    id: *const c_char,
    incremental_id: u64,
    flags: &NodeFlagsHandle,
    connections: &BootstrapConnectionsHandle,
    network_params: &NetworkParamsDto,
) -> *mut BootstrapAttemptHandle {
    let id_str = CStr::from_ptr(id).to_str().unwrap();
    let websocket_server = if websocket_server.is_null() {
        None
    } else {
        Some(Arc::clone((*websocket_server).deref()))
    };
    let bootstrap_initiator = Arc::downgrade(&*bootstrap_initiator);
    let ledger = Arc::clone(&*ledger);
    let network_params = NetworkParams::try_from(network_params).unwrap();
    BootstrapAttemptHandle::new(Arc::new(BootstrapStrategy::Lazy(
        BootstrapAttemptLazy::new(
            self_ptr,
            websocket_server,
            Arc::clone(block_processor),
            bootstrap_initiator,
            ledger,
            id_str,
            incremental_id,
            flags.lock().unwrap().clone(),
            Arc::clone(connections),
            network_params,
        )
        .unwrap(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_lazy_lazy_start(
    handle: &BootstrapAttemptHandle,
    hash_or_account: *const u8,
) -> bool {
    handle
        .as_lazy()
        .lazy_start(&HashOrAccount::from_ptr(hash_or_account))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_lazy_lazy_add(
    handle: &BootstrapAttemptHandle,
    pull: &PullInfoDto,
) {
    handle.as_lazy().lazy_add(&pull.into())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_lazy_lazy_requeue(
    handle: &BootstrapAttemptHandle,
    hash: *const u8,
    previous: *const u8,
) {
    handle
        .as_lazy()
        .lazy_requeue(&BlockHash::from_ptr(hash), &BlockHash::from_ptr(previous));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_lazy_lazy_batch_size(
    handle: &BootstrapAttemptHandle,
) -> u32 {
    handle.as_lazy().lazy_batch_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_lazy_lazy_processed_or_exists(
    handle: &BootstrapAttemptHandle,
    hash: *const u8,
) -> bool {
    handle
        .as_lazy()
        .lazy_processed_or_exists(&BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_lazy_get_information(
    handle: &BootstrapAttemptHandle,
    ptree: *mut c_void,
) {
    let mut writer = FfiPropertyTree::new_borrowed(ptree);
    handle.as_lazy().get_information(&mut writer).unwrap();
}

impl BootstrapAttemptHandle {
    fn as_lazy(&self) -> &BootstrapAttemptLazy {
        let BootstrapStrategy::Lazy(lazy) = self.deref().deref() else {
            panic!("not a lazy bootstrap attempt")
        };
        lazy
    }
}
