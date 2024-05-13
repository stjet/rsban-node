use super::{
    bootstrap_attempt::BootstrapAttemptHandle, bootstrap_attempts::BootstrapAttemptsHandle,
    bootstrap_connections::BootstrapConnectionsHandle, pulls_cache::PullsCacheHandle,
};
use crate::{
    block_processing::BlockProcessorHandle,
    ledger::datastore::LedgerHandle,
    to_rust_string,
    transport::{
        EndpointDto, OutboundBandwidthLimiterHandle, SocketFfiObserver, TcpChannelsHandle,
    },
    utils::{AsyncRuntimeHandle, ContainerInfoComponentHandle, ThreadPoolHandle},
    wallets::AccountVecHandle,
    websocket::WebsocketListenerHandle,
    NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle,
};
use rsnano_core::{Account, HashOrAccount};
use rsnano_node::bootstrap::{BootstrapInitiator, BootstrapInitiatorExt};
use std::{
    ffi::{c_char, c_void, CStr},
    ops::Deref,
    sync::Arc,
};

pub struct BootstrapInitiatorHandle(Arc<BootstrapInitiator>);

impl Deref for BootstrapInitiatorHandle {
    type Target = Arc<BootstrapInitiator>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_create(
    config: &NodeConfigDto,
    flags: &NodeFlagsHandle,
    channels: &TcpChannelsHandle,
    async_rt: &AsyncRuntimeHandle,
    workers: &ThreadPoolHandle,
    network_params: &NetworkParamsDto,
    socket_observer: *mut c_void,
    stats: &StatHandle,
    outbound_limiter: &OutboundBandwidthLimiterHandle,
    block_processor: &BlockProcessorHandle,
    websocket: *mut WebsocketListenerHandle,
    ledger: &LedgerHandle,
) -> *mut BootstrapInitiatorHandle {
    let websocket = if websocket.is_null() {
        None
    } else {
        Some(Arc::clone(&*websocket))
    };
    Box::into_raw(Box::new(BootstrapInitiatorHandle(Arc::new(
        BootstrapInitiator::new(
            config.try_into().unwrap(),
            flags.lock().unwrap().clone(),
            Arc::clone(channels),
            Arc::clone(async_rt),
            Arc::clone(workers),
            network_params.try_into().unwrap(),
            Arc::new(SocketFfiObserver::new(socket_observer)),
            Arc::clone(stats),
            Arc::clone(outbound_limiter),
            Arc::clone(block_processor),
            websocket,
            Arc::clone(&ledger),
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_destroy(handle: *mut BootstrapInitiatorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_initialize(handle: &BootstrapInitiatorHandle) {
    handle.initialize();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_bootstrap(
    handle: &BootstrapInitiatorHandle,
    force: bool,
    id: *const c_char,
    frontiers_age: u32,
    start_account: *const u8,
) {
    handle.bootstrap(
        force,
        to_rust_string(id),
        frontiers_age,
        Account::from_ptr(start_account),
    );
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_bootstrap2(
    handle: &BootstrapInitiatorHandle,
    endpoint: &EndpointDto,
    add_to_peers: bool,
    id: *const c_char,
) {
    handle.bootstrap2(endpoint.into(), add_to_peers, to_rust_string(id));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_bootstrap_lazy(
    handle: &BootstrapInitiatorHandle,
    hash_or_account: *const u8,
    force: bool,
    id: *const c_char,
) -> bool {
    handle.bootstrap_lazy(
        HashOrAccount::from_ptr(hash_or_account),
        force,
        to_rust_string(id),
    )
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_bootstrap_wallet(
    handle: &BootstrapInitiatorHandle,
    accounts: &AccountVecHandle,
) {
    let accounts = accounts.iter().cloned().collect();
    handle.bootstrap_wallet(accounts);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_in_progress(
    handle: &BootstrapInitiatorHandle,
) -> bool {
    handle.in_progress()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_current_attempt(
    handle: &BootstrapInitiatorHandle,
) -> *mut BootstrapAttemptHandle {
    match handle.current_attempt() {
        Some(attempt) => BootstrapAttemptHandle::new(attempt),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_current_lazy_attempt(
    handle: &BootstrapInitiatorHandle,
) -> *mut BootstrapAttemptHandle {
    match handle.current_lazy_attempt() {
        Some(attempt) => BootstrapAttemptHandle::new(attempt),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_current_wallet_attempt(
    handle: &BootstrapInitiatorHandle,
) -> *mut BootstrapAttemptHandle {
    match handle.current_wallet_attempt() {
        Some(attempt) => BootstrapAttemptHandle::new(attempt),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_start(handle: &BootstrapInitiatorHandle) {
    handle.start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_stop(handle: &BootstrapInitiatorHandle) {
    handle.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_attempts(
    handle: &BootstrapInitiatorHandle,
) -> *mut BootstrapAttemptsHandle {
    BootstrapAttemptsHandle::new(Arc::clone(&handle.attempts))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_connections(
    handle: &BootstrapInitiatorHandle,
) -> *mut BootstrapConnectionsHandle {
    BootstrapConnectionsHandle::new(Arc::clone(&handle.connections))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_cache(
    handle: &BootstrapInitiatorHandle,
) -> *mut PullsCacheHandle {
    PullsCacheHandle::new(Arc::clone(&handle.cache))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_collect_container_info(
    handle: &BootstrapInitiatorHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = handle
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
