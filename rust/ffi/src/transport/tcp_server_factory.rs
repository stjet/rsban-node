use super::{ChannelHandle, NetworkFilterHandle, SocketHandle, TcpMessageManagerHandle};
use crate::{
    bootstrap::{FfiBootstrapServerObserver, RequestResponseVisitorFactoryHandle, TcpServerHandle},
    core::BlockUniquerHandle,
    utils::{FfiIoContext, IoContextHandle, LoggerHandle, LoggerMT},
    voting::VoteUniquerHandle,
    NetworkParamsDto, NodeConfigDto, StatHandle,
};
use rsnano_core::utils::Logger;
use rsnano_node::{
    config::NodeConfig,
    transport::{ChannelEnum, NullTcpServerObserver, TcpServerFactory},
    NetworkParams,
};
use std::{ffi::c_void, sync::Arc};

pub struct TcpServerFactoryHandle(TcpServerFactory);

#[repr(C)]
pub struct TcpServerFactoryParams {
    pub node_config: *const NodeConfigDto,
    pub logger: *mut LoggerHandle,
    pub publish_filter: *mut NetworkFilterHandle,
    pub io_ctx: *mut IoContextHandle,
    pub network: *mut NetworkParamsDto,
    pub stats: *mut StatHandle,
    pub block_uniquer: *mut BlockUniquerHandle,
    pub vote_uniquer: *mut VoteUniquerHandle,
    pub tcp_message_manager: *mut TcpMessageManagerHandle,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_server_factory_create(
    params: &mut TcpServerFactoryParams,
) -> *mut TcpServerFactoryHandle {
    let config = Arc::new(NodeConfig::try_from(&*params.node_config).unwrap());
    let logger: Arc<dyn Logger> = Arc::new(LoggerMT::new(Box::from_raw(params.logger)));
    let io_ctx = Arc::new(FfiIoContext::new((*params.io_ctx).raw_handle()));
    let network = Arc::new(NetworkParams::try_from(&*params.network).unwrap());
    let stats = Arc::clone(&(*params.stats).0);
    let block_uniquer = Arc::clone(&(*params.block_uniquer));
    let vote_uniquer = Arc::clone(&(*params.vote_uniquer));
    let tcp_message_manager = Arc::clone(&(*params.tcp_message_manager));

    Box::into_raw(Box::new(TcpServerFactoryHandle(TcpServerFactory {
        config,
        logger,
        observer: Arc::new(NullTcpServerObserver {}),
        publish_filter: Arc::clone(&(*params.publish_filter).0),
        io_ctx,
        network,
        stats,
        block_uniquer,
        vote_uniquer,
        tcp_message_manager,
        message_visitor_factory: None,
    })))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_server_factory_set_message_visitor_factory(
    handle: *mut TcpServerFactoryHandle,
    visitor_factory: *mut RequestResponseVisitorFactoryHandle,
) {
    (*handle).0.message_visitor_factory = Some(Arc::clone(&(*visitor_factory).0))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_server_factory_destroy(handle: *mut TcpServerFactoryHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_server_factory_set_observer(
    handle: *mut TcpServerFactoryHandle,
    observer: *mut c_void,
) {
    let observer = Arc::new(FfiBootstrapServerObserver::new(observer));
    (*handle).0.observer = observer;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_server_factory_create_tcp_server(
    handle: *mut TcpServerFactoryHandle,
    channel: &ChannelHandle,
    socket: &SocketHandle,
) -> *mut TcpServerHandle {
    let ChannelEnum::Tcp(tcp) = channel.0.as_ref() else { panic!("not a tcp channel!")};
    let socket = Arc::clone(&socket.0);
    TcpServerHandle::new((*handle).0.create_tcp_server(tcp, socket))
}
