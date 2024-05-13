use super::{
    bootstrap_attempts::BootstrapAttemptsHandle, pulls_cache::PullsCacheHandle,
    BootstrapInitiatorHandle,
};
use crate::{
    block_processing::BlockProcessorHandle,
    transport::{
        EndpointDto, OutboundBandwidthLimiterHandle, SocketFfiObserver, TcpChannelsHandle,
    },
    utils::{AsyncRuntimeHandle, ThreadPoolHandle},
    FfiPropertyTree, NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle,
};
use rsnano_node::bootstrap::{BootstrapConnections, BootstrapConnectionsExt};
use std::{ffi::c_void, ops::Deref, sync::Arc};

pub struct BootstrapConnectionsHandle(Arc<BootstrapConnections>);

impl Deref for BootstrapConnectionsHandle {
    type Target = Arc<BootstrapConnections>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_create(
    attempts: &BootstrapAttemptsHandle,
    config: &NodeConfigDto,
    flags: &NodeFlagsHandle,
    channels: &TcpChannelsHandle,
    async_rt: &AsyncRuntimeHandle,
    workers: &ThreadPoolHandle,
    network_params: &NetworkParamsDto,
    observers: *mut c_void,
    stats: &StatHandle,
    outbound_limiter: &OutboundBandwidthLimiterHandle,
    block_processor: &BlockProcessorHandle,
    bootstrap_initiator: &BootstrapInitiatorHandle,
    pulls_cache: &PullsCacheHandle,
) -> *mut BootstrapConnectionsHandle {
    let ffi_observer = Arc::new(SocketFfiObserver::new(observers));
    Box::into_raw(Box::new(BootstrapConnectionsHandle(Arc::new(
        BootstrapConnections::new(
            Arc::clone(attempts),
            config.try_into().unwrap(),
            flags.lock().unwrap().clone(),
            Arc::clone(channels),
            Arc::clone(async_rt),
            Arc::clone(workers),
            network_params.try_into().unwrap(),
            ffi_observer,
            Arc::clone(stats),
            Arc::clone(&outbound_limiter),
            Arc::clone(&block_processor),
            Arc::clone(&bootstrap_initiator),
            Arc::clone(&pulls_cache),
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_connections_drop(handle: *mut BootstrapConnectionsHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_add_connection(
    handle: &BootstrapConnectionsHandle,
    endpoint: &EndpointDto,
) {
    handle.add_connection(endpoint.into());
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_target_connections(
    handle: &BootstrapConnectionsHandle,
    pulls_remaining: usize,
    attempts_count: usize,
) -> u32 {
    handle.target_connections(pulls_remaining, attempts_count)
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_clear_pulls(
    handle: &BootstrapConnectionsHandle,
    bootstrap_id: u64,
) {
    handle.clear_pulls(bootstrap_id)
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_run(handle: &BootstrapConnectionsHandle) {
    handle.run();
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_stop(handle: &BootstrapConnectionsHandle) {
    handle.stop();
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_bootstrap_status(
    handle: &BootstrapConnectionsHandle,
    tree: *mut c_void,
    attempts_count: usize,
) {
    handle.bootstrap_status(&mut FfiPropertyTree::new_borrowed(tree), attempts_count);
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_connections_count(
    handle: &BootstrapConnectionsHandle,
) -> u32 {
    handle
        .connections_count
        .load(std::sync::atomic::Ordering::SeqCst)
}
