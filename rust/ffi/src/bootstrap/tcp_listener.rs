use super::TcpServerHandle;
use crate::{
    transport::{
        EndpointDto, ServerSocketHandle, SocketFfiObserver, SocketHandle, SynCookiesHandle,
        TcpChannelsHandle,
    },
    utils::{AsyncRuntimeHandle, ContextWrapper, LoggerHandle, LoggerMT, ThreadPoolHandle},
    ErrorCodeDto, NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle,
    VoidPointerCallback,
};
use rsnano_core::utils::Logger;
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
) -> *mut TcpListenerHandle {
    let logger: Arc<dyn Logger> = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let ffi_observer = Arc::new(SocketFfiObserver::new(callback_handler));
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

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_connections_clear(handle: &mut TcpListenerHandle) {
    handle.0.clear_connections();
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_is_on(handle: &TcpListenerHandle) -> bool {
    handle.0.is_on()
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_set_on(handle: &mut TcpListenerHandle) {
    handle.0.set_on();
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_set_off(handle: &mut TcpListenerHandle) {
    handle.0.set_off();
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_has_listening_socket(handle: &TcpListenerHandle) -> bool {
    handle.0.has_listening_socket()
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_close_listening_socket(handle: &mut TcpListenerHandle) {
    handle.0.close_listening_socket();
}
