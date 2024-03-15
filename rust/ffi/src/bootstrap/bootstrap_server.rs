use super::{
    request_response_visitor_factory::RequestResponseVisitorFactoryHandle, TcpListenerHandle,
};
use crate::{
    messages::MessageHandle,
    transport::{EndpointDto, NetworkFilterHandle, SocketHandle, TcpMessageManagerHandle},
    utils::AsyncRuntimeHandle,
    NetworkParamsDto, NodeConfigDto, StatHandle,
};
use rsnano_core::Account;
use rsnano_messages::{DeserializedMessage, Message, ProtocolInfo};
use rsnano_node::{
    config::NodeConfig,
    transport::{TcpServer, TcpServerExt},
    NetworkParams,
};
use std::{ops::Deref, sync::Arc};

pub struct TcpServerHandle(pub Arc<TcpServer>);

impl TcpServerHandle {
    pub fn new(server: Arc<TcpServer>) -> *mut TcpServerHandle {
        Box::into_raw(Box::new(TcpServerHandle(server)))
    }
}

impl Deref for TcpServerHandle {
    type Target = Arc<TcpServer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(C)]
pub struct CreateTcpServerParams {
    pub async_rt: *mut AsyncRuntimeHandle,
    pub socket: *mut SocketHandle,
    pub config: *const NodeConfigDto,
    pub observer: *mut TcpListenerHandle,
    pub publish_filter: *mut NetworkFilterHandle,
    pub network: *const NetworkParamsDto,
    pub disable_bootstrap_listener: bool,
    pub connections_max: usize,
    pub stats: *mut StatHandle,
    pub disable_bootstrap_bulk_pull_server: bool,
    pub disable_tcp_realtime: bool,
    pub request_response_visitor_factory: *mut RequestResponseVisitorFactoryHandle,
    pub tcp_message_manager: *mut TcpMessageManagerHandle,
    pub allow_bootstrap: bool,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_create(
    params: &CreateTcpServerParams,
) -> *mut TcpServerHandle {
    let async_rt = Arc::clone(&(*params.async_rt).0);
    let socket = Arc::clone(&(*params.socket));
    let config = Arc::new(NodeConfig::try_from(&*params.config).unwrap());
    let observer = Arc::downgrade(&*params.observer);
    let publish_filter = Arc::clone(&*params.publish_filter);
    let network = Arc::new(NetworkParams::try_from(&*params.network).unwrap());
    let stats = Arc::clone(&(*params.stats));
    let visitor_factory = Arc::clone(&(*params.request_response_visitor_factory).0);
    let tcp_message_manager = Arc::clone(&*params.tcp_message_manager);
    let mut server = TcpServer::new(
        async_rt,
        socket,
        config,
        observer,
        publish_filter,
        network,
        stats,
        tcp_message_manager,
        visitor_factory,
        params.allow_bootstrap,
    );
    server.disable_bootstrap_listener = params.disable_bootstrap_listener;
    server.connections_max = params.connections_max;
    server.disable_bootstrap_bulk_pull_server = params.disable_bootstrap_bulk_pull_server;
    server.disable_tcp_realtime = params.disable_tcp_realtime;
    TcpServerHandle::new(Arc::new(server))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_destroy(handle: *mut TcpServerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_unique_id(handle: *mut TcpServerHandle) -> usize {
    (*handle).unique_id()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_start(handle: *mut TcpServerHandle) {
    (*handle).start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_stop(handle: *mut TcpServerHandle) {
    (*handle).stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_is_stopped(handle: *mut TcpServerHandle) -> bool {
    (*handle).is_stopped()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_remote_endpoint(
    handle: *mut TcpServerHandle,
    endpoint: *mut EndpointDto,
) {
    (*endpoint) = (*handle).remote_endpoint().into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_set_remote_node_id(
    handle: *mut TcpServerHandle,
    node_id: *const u8,
) {
    let mut lk = (*handle).remote_node_id.lock().unwrap();
    *lk = Account::from_ptr(node_id);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_socket(
    handle: *mut TcpServerHandle,
) -> *mut SocketHandle {
    SocketHandle::new((*handle).socket.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_timeout(handle: *mut TcpServerHandle) {
    (*handle).timeout();
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_server_get_last_keepalive(
    handle: &TcpServerHandle,
) -> *mut MessageHandle {
    match handle.get_last_keepalive() {
        Some(keepalive) => MessageHandle::new(DeserializedMessage::new(
            Message::Keepalive(keepalive),
            ProtocolInfo::default(),
        )),
        None => std::ptr::null_mut(),
    }
}
