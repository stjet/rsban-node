use super::TcpServerHandle;
use crate::{
    transport::{ServerSocketHandle, SynCookiesHandle, TcpChannelsHandle},
    utils::{LoggerHandle, LoggerMT},
    NodeConfigDto,
};
use rsnano_core::utils::Logger;
use rsnano_node::transport::TcpListener;
use std::sync::Arc;

pub struct TcpListenerHandle(TcpListener);

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_listener_create(
    port: u16,
    max_inbound_connections: usize,
    config: &NodeConfigDto,
    logger: *mut LoggerHandle,
    tcp_channels: &TcpChannelsHandle,
    syn_cookies: &SynCookiesHandle,
) -> *mut TcpListenerHandle {
    let logger: Arc<dyn Logger> = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    Box::into_raw(Box::new(TcpListenerHandle(TcpListener::new(
        port,
        max_inbound_connections,
        config.try_into().unwrap(),
        logger,
        Arc::clone(tcp_channels),
        Arc::clone(syn_cookies),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_listener_destroy(handle: *mut TcpListenerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_connections_add(
    handle: &mut TcpListenerHandle,
    connection: &TcpServerHandle,
) {
    handle.0.add_connection(connection);
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_connections_erase(
    handle: &mut TcpListenerHandle,
    conn_id: usize,
) {
    handle.0.remove_connection(conn_id);
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_connections_len(handle: &TcpListenerHandle) -> usize {
    handle.0.connection_count()
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
pub extern "C" fn rsn_tcp_listener_set_listening_socket(
    handle: &mut TcpListenerHandle,
    socket: &ServerSocketHandle,
) {
    handle.0.set_listening_socket(Arc::clone(&socket.0));
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_close_listening_socket(handle: &mut TcpListenerHandle) {
    handle.0.close_listening_socket();
}
