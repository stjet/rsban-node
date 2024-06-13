use super::{
    request_response_visitor_factory::RequestResponseVisitorFactoryHandle, TcpListenerHandle,
};
use crate::{
    transport::{
        NetworkFilterHandle, SocketHandle, SynCookiesHandle, TcpChannelsHandle,
        TcpMessageManagerHandle,
    },
    utils::AsyncRuntimeHandle,
    NetworkParamsDto, NodeConfigDto, StatHandle,
};
use rsnano_core::KeyPair;
use rsnano_node::{config::NodeConfig, transport::ResponseServerImpl, NetworkParams};
use std::{ops::Deref, sync::Arc};

pub struct TcpServerHandle(pub Arc<ResponseServerImpl>);

impl TcpServerHandle {
    pub fn new(server: Arc<ResponseServerImpl>) -> *mut TcpServerHandle {
        Box::into_raw(Box::new(TcpServerHandle(server)))
    }
}

impl Deref for TcpServerHandle {
    type Target = Arc<ResponseServerImpl>;

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
    pub syn_cookies: *mut SynCookiesHandle,
    pub node_id_priv: *const u8,
    pub tcp_channels: *mut TcpChannelsHandle,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_server_create(
    params: &CreateTcpServerParams,
) -> *mut TcpServerHandle {
    let socket = Arc::clone(&(*params.socket));
    let config = Arc::new(NodeConfig::try_from(&*params.config).unwrap());
    let publish_filter = Arc::clone(&*params.publish_filter);
    let network = Arc::new(NetworkParams::try_from(&*params.network).unwrap());
    let stats = Arc::clone(&(*params.stats));
    let visitor_factory = Arc::clone(&(*params.request_response_visitor_factory).0);
    let tcp_message_manager = Arc::clone(&*params.tcp_message_manager);
    let channels = Arc::clone(&(*params.tcp_channels));
    let mut server = ResponseServerImpl::new(
        &channels,
        socket,
        config,
        publish_filter,
        network,
        stats,
        tcp_message_manager,
        visitor_factory,
        params.allow_bootstrap,
        Arc::clone(&*params.syn_cookies),
        KeyPair::from_priv_key_bytes(std::slice::from_raw_parts(params.node_id_priv, 32)).unwrap(),
    );
    server.disable_bootstrap_listener = params.disable_bootstrap_listener;
    server.connections_max = params.connections_max;
    server.disable_bootstrap_bulk_pull_server = params.disable_bootstrap_bulk_pull_server;
    TcpServerHandle::new(Arc::new(server))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_server_destroy(handle: *mut TcpServerHandle) {
    drop(Box::from_raw(handle))
}
