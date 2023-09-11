use rsnano_node::{
    config::NodeConfig,
    transport::{
        ServerSocket, ServerSocketExtensions, Socket, TcpSocketFacade, TcpSocketFacadeFactory,
        TokioSocketFacade, TokioSocketFacadeFactory,
    },
    utils::{is_tokio_enabled, ErrorCode},
    NetworkParams,
};
use std::{
    ffi::c_void,
    net::{SocketAddr, SocketAddrV6},
    sync::Arc,
};

use super::{
    socket::FfiTcpSocketFacadeFactory, EndpointDto, FfiTcpSocketFacade, SocketFfiObserver,
    SocketHandle,
};
use crate::{
    utils::{AsyncRuntimeHandle, ContextWrapper, LoggerHandle, LoggerMT, ThreadPoolHandle},
    ErrorCodeDto, NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle,
    VoidPointerCallback,
};
pub struct ServerSocketHandle(Arc<ServerSocket>);

#[no_mangle]
pub unsafe extern "C" fn rsn_server_socket_create(
    socket_facade_ptr: *mut c_void,
    flags: &NodeFlagsHandle,
    network_params: &NetworkParamsDto,
    workers: &ThreadPoolHandle,
    logger: *mut LoggerHandle,
    tcp_socket_facade_factory_handle: *mut c_void,
    callback_handler: *mut c_void,
    stats: &StatHandle,
    node_config: &NodeConfigDto,
    max_inbound_connections: usize,
    local: &EndpointDto,
    async_rt: &AsyncRuntimeHandle,
) -> *mut ServerSocketHandle {
    let logger = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let mut socket_facade: Arc<dyn TcpSocketFacade> =
        Arc::new(FfiTcpSocketFacade::new(socket_facade_ptr));
    let network_params = NetworkParams::try_from(network_params).unwrap();
    let mut tcp_socket_facade_factory: Arc<dyn TcpSocketFacadeFactory> =
        Arc::new(FfiTcpSocketFacadeFactory(tcp_socket_facade_factory_handle));

    if is_tokio_enabled() {
        socket_facade = Arc::new(TokioSocketFacade::new(Arc::clone(&async_rt.0)));
        tcp_socket_facade_factory =
            Arc::new(TokioSocketFacadeFactory::new(Arc::clone(&async_rt.0)));
    }
    let ffi_observer = Arc::new(SocketFfiObserver::new(callback_handler));
    let stats = Arc::clone(&stats.0);
    let node_config = NodeConfig::try_from(node_config).unwrap();
    Box::into_raw(Box::new(ServerSocketHandle(Arc::new(ServerSocket::new(
        socket_facade,
        flags.0.lock().unwrap().clone(),
        network_params,
        Arc::clone(&workers.0),
        logger,
        tcp_socket_facade_factory,
        node_config,
        stats,
        ffi_observer,
        max_inbound_connections,
        local.into(),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_server_socket_destroy(handle: *mut ServerSocketHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_server_socket_close(handle: &mut ServerSocketHandle) {
    handle.0.close();
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

#[no_mangle]
pub extern "C" fn rsn_server_socket_limit_reached_for_incoming_subnetwork_connections(
    handle: &ServerSocketHandle,
    new_conenction: &SocketHandle,
) -> bool {
    handle
        .0
        .limit_reached_for_incoming_subnetwork_connections(&new_conenction.0)
}

#[no_mangle]
pub extern "C" fn rsn_server_socket_limit_reached_for_incoming_ip_connections(
    handle: &ServerSocketHandle,
    new_conenction: &SocketHandle,
) -> bool {
    handle
        .0
        .limit_reached_for_incoming_ip_connections(&new_conenction.0)
}

pub type OnConnectionCallback =
    extern "C" fn(*mut c_void, *mut SocketHandle, *const ErrorCodeDto) -> bool;

#[no_mangle]
pub extern "C" fn rsn_server_socket_on_connection(
    handle: &ServerSocketHandle,
    callback: OnConnectionCallback,
    callback_context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context = ContextWrapper::new(callback_context, delete_context);
    let callback_wrapper = Box::new(move |socket: Arc<Socket>, ec: ErrorCode| {
        let ec_dto = ErrorCodeDto::from(&ec);
        callback(context.get_context(), SocketHandle::new(socket), &ec_dto)
    });
    handle.0.on_connection(callback_wrapper);
}

#[no_mangle]
pub extern "C" fn rsn_server_socket_start(handle: &mut ServerSocketHandle) {
    handle.0.start();
}

#[no_mangle]
pub extern "C" fn rsn_server_socket_listening_port(handle: &mut ServerSocketHandle) -> u16 {
    handle.0.listening_port()
}
