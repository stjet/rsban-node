use crate::{
    block_processing::UncheckedMapHandle,
    fill_node_config_dto,
    ledger::datastore::{lmdb::LmdbStoreHandle, LedgerHandle},
    to_rust_string,
    transport::{
        NetworkFilterHandle, OutboundBandwidthLimiterHandle, SocketFfiObserver, SynCookiesHandle,
        TcpChannelsHandle, TcpMessageManagerHandle,
    },
    utils::{AsyncRuntimeHandle, ThreadPoolHandle},
    work::{DistributedWorkFactoryHandle, WorkPoolHandle},
    NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle,
};
use rsnano_node::node::Node;
use std::{
    ffi::{c_char, c_void},
    sync::Arc,
};

pub struct NodeHandle(Arc<Node>);

#[no_mangle]
pub unsafe extern "C" fn rsn_node_create(
    path: *const c_char,
    async_rt: &AsyncRuntimeHandle,
    config: &NodeConfigDto,
    params: &NetworkParamsDto,
    flags: &NodeFlagsHandle,
    work: &WorkPoolHandle,
    socket_observer: *mut c_void,
) -> *mut NodeHandle {
    let path = to_rust_string(path);
    let observer = Arc::new(SocketFfiObserver::new(socket_observer));
    Box::into_raw(Box::new(NodeHandle(Arc::new(Node::new(
        Arc::clone(async_rt),
        path,
        config.try_into().unwrap(),
        params.try_into().unwrap(),
        flags.lock().unwrap().clone(),
        Arc::clone(work),
        observer,
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_node_destroy(handle: *mut NodeHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_node_node_id(handle: &NodeHandle, result: *mut u8) {
    handle.0.node_id.private_key().copy_bytes(result);
}

#[no_mangle]
pub extern "C" fn rsn_node_config(handle: &NodeHandle, result: &mut NodeConfigDto) {
    fill_node_config_dto(result, &handle.0.config);
}

#[no_mangle]
pub extern "C" fn rsn_node_stats(handle: &NodeHandle) -> *mut StatHandle {
    StatHandle::new(&Arc::clone(&handle.0.stats))
}

#[no_mangle]
pub extern "C" fn rsn_node_workers(handle: &NodeHandle) -> *mut ThreadPoolHandle {
    Box::into_raw(Box::new(ThreadPoolHandle(Arc::clone(&handle.0.workers))))
}

#[no_mangle]
pub extern "C" fn rsn_node_bootstrap_workers(handle: &NodeHandle) -> *mut ThreadPoolHandle {
    Box::into_raw(Box::new(ThreadPoolHandle(Arc::clone(
        &handle.0.bootstrap_workers,
    ))))
}

#[no_mangle]
pub extern "C" fn rsn_node_distributed_work(
    handle: &NodeHandle,
) -> *mut DistributedWorkFactoryHandle {
    Box::into_raw(Box::new(DistributedWorkFactoryHandle(Arc::clone(
        &handle.0.distributed_work,
    ))))
}

#[no_mangle]
pub extern "C" fn rsn_node_store(handle: &NodeHandle) -> *mut LmdbStoreHandle {
    Box::into_raw(Box::new(LmdbStoreHandle(Arc::clone(&handle.0.store))))
}

#[no_mangle]
pub extern "C" fn rsn_node_unchecked(handle: &NodeHandle) -> *mut UncheckedMapHandle {
    Box::into_raw(Box::new(UncheckedMapHandle(Arc::clone(
        &handle.0.unchecked,
    ))))
}

#[no_mangle]
pub extern "C" fn rsn_node_ledger(handle: &NodeHandle) -> *mut LedgerHandle {
    Box::into_raw(Box::new(LedgerHandle(Arc::clone(&handle.0.ledger))))
}

#[no_mangle]
pub extern "C" fn rsn_node_outbound_bandwidth_limiter(
    handle: &NodeHandle,
) -> *mut OutboundBandwidthLimiterHandle {
    Box::into_raw(Box::new(OutboundBandwidthLimiterHandle(Arc::clone(
        &handle.0.outbound_limiter,
    ))))
}

#[no_mangle]
pub extern "C" fn rsn_node_syn_cookies(handle: &NodeHandle) -> *mut SynCookiesHandle {
    Box::into_raw(Box::new(SynCookiesHandle(Arc::clone(
        &handle.0.syn_cookies,
    ))))
}

#[no_mangle]
pub extern "C" fn rsn_node_tcp_channels(handle: &NodeHandle) -> *mut TcpChannelsHandle {
    Box::into_raw(Box::new(TcpChannelsHandle(Arc::clone(&handle.0.channels))))
}

#[no_mangle]
pub extern "C" fn rsn_node_tcp_message_manager(
    handle: &NodeHandle,
) -> *mut TcpMessageManagerHandle {
    Box::into_raw(Box::new(TcpMessageManagerHandle(Arc::clone(
        &handle.0.channels.tcp_message_manager,
    ))))
}

#[no_mangle]
pub extern "C" fn rsn_node_network_filter(handle: &NodeHandle) -> *mut NetworkFilterHandle {
    Box::into_raw(Box::new(NetworkFilterHandle(Arc::clone(
        &handle.0.channels.publish_filter,
    ))))
}
