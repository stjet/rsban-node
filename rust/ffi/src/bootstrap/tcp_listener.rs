use super::{BootstrapInitiatorHandle, TcpServerHandle};
use crate::{
    block_processing::BlockProcessorHandle,
    ledger::datastore::LedgerHandle,
    transport::{
        EndpointDto, SocketFfiObserver, SocketHandle, SynCookiesHandle, TcpChannelsHandle,
        TcpMessageManagerHandle,
    },
    utils::{AsyncRuntimeHandle, ContextWrapper, LoggerHandle, LoggerMT, ThreadPoolHandle},
    ErrorCodeDto, NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle,
    VoidPointerCallback,
};
use rsnano_core::{utils::Logger, KeyPair};
use rsnano_node::{
    transport::{Socket, TcpListener},
    utils::ErrorCode,
};
use std::{ffi::c_void, sync::Arc};

pub struct TcpListenerHandle(TcpListener);

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_listener_create(
    port: u16,
    max_inbound_connections: usize,
    config: &NodeConfigDto,
    logger: *mut LoggerHandle,
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
    let logger: Arc<dyn Logger> = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let ffi_observer = Arc::new(SocketFfiObserver::new(callback_handler));
    let node_id = Arc::new(
        KeyPair::from_priv_key_bytes(std::slice::from_raw_parts(node_id_prv, 32)).unwrap(),
    );
    Box::into_raw(Box::new(TcpListenerHandle(TcpListener::new(
        port,
        max_inbound_connections,
        config.try_into().unwrap(),
        logger,
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
    ))))
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

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_connections_erase(
    handle: &mut TcpListenerHandle,
    conn_id: usize,
) {
    handle.0.remove_connection(conn_id);
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_connections_add(
    handle: &mut TcpListenerHandle,
    connection: &TcpServerHandle,
) {
    handle.0.add_connection(connection);
}
