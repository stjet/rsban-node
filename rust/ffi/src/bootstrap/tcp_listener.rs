use super::BootstrapInitiatorHandle;
use crate::{
    block_processing::BlockProcessorHandle,
    ledger::datastore::LedgerHandle,
    transport::{
        EndpointDto, SocketFfiObserver, SocketHandle, SynCookiesHandle, TcpChannelsHandle,
    },
    utils::{AsyncRuntimeHandle, ContextWrapper, ThreadPoolHandle},
    ErrorCodeDto, NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle,
    VoidPointerCallback,
};
use rsnano_core::KeyPair;
use rsnano_node::{
    transport::{Socket, TcpListener, TcpListenerExt},
    utils::ErrorCode,
};
use std::{ffi::c_void, ops::Deref, sync::Arc};

pub struct TcpListenerHandle(Arc<TcpListener>);

impl Deref for TcpListenerHandle {
    type Target = Arc<TcpListener>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_listener_create(
    port: u16,
    max_inbound_connections: usize,
    config: &NodeConfigDto,
    tcp_channels: &TcpChannelsHandle,
    syn_cookies: &SynCookiesHandle,
    network_params: &NetworkParamsDto,
    node_flags: &NodeFlagsHandle,
    runtime: &AsyncRuntimeHandle,
    stats: &StatHandle,
    workers: &ThreadPoolHandle,
    callback_handler: *mut c_void,
    block_processor: &BlockProcessorHandle,
    bootstrap_initiator: &BootstrapInitiatorHandle,
    ledger: &LedgerHandle,
    node_id_prv: *const u8,
) -> *mut TcpListenerHandle {
    let ffi_observer = Arc::new(SocketFfiObserver::new(callback_handler));
    let node_id = Arc::new(
        KeyPair::from_priv_key_bytes(std::slice::from_raw_parts(node_id_prv, 32)).unwrap(),
    );
    Box::into_raw(Box::new(TcpListenerHandle(Arc::new(TcpListener::new(
        port,
        max_inbound_connections,
        config.try_into().unwrap(),
        Arc::clone(tcp_channels),
        Arc::clone(syn_cookies),
        network_params.try_into().unwrap(),
        node_flags.lock().unwrap().clone(),
        Arc::clone(runtime),
        ffi_observer,
        Arc::clone(stats),
        Arc::clone(workers),
        Arc::clone(block_processor),
        Arc::clone(bootstrap_initiator),
        Arc::clone(ledger),
        node_id,
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_listener_destroy(handle: *mut TcpListenerHandle) {
    drop(Box::from_raw(handle))
}

pub type OnConnectionCallback =
    extern "C" fn(*mut c_void, *mut SocketHandle, *const ErrorCodeDto) -> bool;

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_start(
    handle: &mut TcpListenerHandle,
    callback: OnConnectionCallback,
    callback_context: *mut c_void,
    delete_context: VoidPointerCallback,
) -> bool {
    let context = ContextWrapper::new(callback_context, delete_context);
    let callback_wrapper = Box::new(move |socket: Arc<Socket>, ec: ErrorCode| {
        let ec_dto = ErrorCodeDto::from(&ec);
        callback(context.get_context(), SocketHandle::new(socket), &ec_dto)
    });
    handle.0.start(callback_wrapper).is_ok()
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_accept_action(
    handle: &mut TcpListenerHandle,
    ec: &ErrorCodeDto,
    socket: &SocketHandle,
) {
    handle.0.accept_action(ec.into(), Arc::clone(socket));
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_realtime_count(handle: &TcpListenerHandle) -> usize {
    handle.0.get_realtime_count()
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_stop(handle: &mut TcpListenerHandle) {
    handle.0.stop()
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_connection_count(handle: &TcpListenerHandle) -> usize {
    handle.0.connection_count()
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_endpoint(handle: &TcpListenerHandle, result: &mut EndpointDto) {
    *result = handle.0.endpoint().into()
}
