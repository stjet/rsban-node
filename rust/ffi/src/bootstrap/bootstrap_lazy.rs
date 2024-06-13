use super::{
    bootstrap_attempt::BootstrapAttemptHandle, bootstrap_connections::BootstrapConnectionsHandle,
    bootstrap_initiator::BootstrapInitiatorHandle,
};
use crate::{
    block_processing::BlockProcessorHandle, ledger::datastore::LedgerHandle,
    websocket::WebsocketListenerHandle, FfiPropertyTree, NetworkParamsDto, NodeFlagsHandle,
};
use rsnano_node::{
    bootstrap::{BootstrapAttemptLazy, BootstrapStrategy},
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
            websocket_server,
            Arc::clone(block_processor),
            bootstrap_initiator,
            ledger,
            id_str.to_string(),
            incremental_id,
            flags.lock().unwrap().clone(),
            Arc::clone(connections),
            network_params,
        )
        .unwrap(),
    )))
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
