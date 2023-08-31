use rsnano_node::transport::ServerSocket;
use std::{
    ffi::c_void,
    net::{SocketAddr, SocketAddrV6},
    sync::Arc,
};

use super::{EndpointDto, FfiTcpSocketFacade, SocketHandle};

pub struct ServerSocketHandle(Arc<ServerSocket>);

#[no_mangle]
pub extern "C" fn rsn_server_socket_create(
    socket_facade_ptr: *mut c_void,
) -> *mut ServerSocketHandle {
    let socket_facade = Arc::new(FfiTcpSocketFacade::new(socket_facade_ptr));
    Box::into_raw(Box::new(ServerSocketHandle(Arc::new(ServerSocket::new(
        socket_facade,
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_server_socket_destroy(handle: *mut ServerSocketHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_server_socket_close_connections(handle: &mut ServerSocketHandle) {
    handle.0.close_connections();
}

#[no_mangle]
pub extern "C" fn rsn_server_socket_count_subnetwork_connections(
    handle: &ServerSocketHandle,
    endpoint: &EndpointDto,
    ipv6_subnetwork_prefix_for_limiting: usize,
) -> usize {
    let address = SocketAddrV6::from(endpoint);
    handle
        .0
        .count_subnetwork_connections(address.ip(), ipv6_subnetwork_prefix_for_limiting)
}

#[no_mangle]
pub extern "C" fn rsn_server_socket_count_connections_for_ip(
    handle: &ServerSocketHandle,
    endpoint: &EndpointDto,
) -> usize {
    let endpoint = SocketAddr::from(endpoint);
    let ip = match endpoint.ip() {
        std::net::IpAddr::V4(ip) => ip.to_ipv6_mapped(),
        std::net::IpAddr::V6(ip) => ip,
    };
    handle.0.count_connections_for_ip(&ip)
}

#[no_mangle]
pub extern "C" fn rsn_server_socket_count_connections(handle: &ServerSocketHandle) -> usize {
    handle.0.count_connections()
}

#[no_mangle]
pub extern "C" fn rsn_server_socket_insert_connection(
    handle: &ServerSocketHandle,
    connection: &SocketHandle,
) {
    handle.0.insert_connection(&connection.0);
}

#[no_mangle]
pub extern "C" fn rsn_server_socket_evict_dead_connections(handle: &ServerSocketHandle) {
    handle.0.evict_dead_connections();
}
