use rsnano_node::{
    config::NodeConfig,
    transport::{
        ServerSocket, ServerSocketExtensions, Socket, TokioSocketFacade, TokioSocketFacadeFactory,
    },
    utils::ErrorCode,
    NetworkParams,
};
use std::{ffi::c_void, sync::Arc};

use super::{EndpointDto, SocketFfiObserver, SocketHandle};
use crate::{
    utils::{AsyncRuntimeHandle, ContextWrapper, LoggerHandle, LoggerMT, ThreadPoolHandle},
    ErrorCodeDto, NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle,
    VoidPointerCallback,
};
pub struct ServerSocketHandle(Arc<ServerSocket>);

#[no_mangle]
pub unsafe extern "C" fn rsn_server_socket_create(
    flags: &NodeFlagsHandle,
    network_params: &NetworkParamsDto,
    workers: &ThreadPoolHandle,
    logger: *mut LoggerHandle,
    callback_handler: *mut c_void,
    stats: &StatHandle,
    node_config: &NodeConfigDto,
    max_inbound_connections: usize,
    local: &EndpointDto,
    async_rt: &AsyncRuntimeHandle,
) -> *mut ServerSocketHandle {
    let logger = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let network_params = NetworkParams::try_from(network_params).unwrap();
    let socket_facade = Arc::new(TokioSocketFacade::create(Arc::clone(&async_rt.0)));
    let tcp_socket_facade_factory =
        Arc::new(TokioSocketFacadeFactory::new(Arc::clone(&async_rt.0)));

    let ffi_observer = Arc::new(SocketFfiObserver::new(callback_handler));
    let stats = Arc::clone(&stats.0);
    let node_config = NodeConfig::try_from(node_config).unwrap();
    let runtime = Arc::downgrade(&async_rt.0);
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
        runtime,
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
