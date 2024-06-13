use super::{
    bootstrap_attempt::BootstrapAttemptHandle, bootstrap_client::BootstrapClientHandle,
    bootstrap_connections::BootstrapConnectionsHandle, pulls_cache::PullInfoDto,
    BootstrapInitiatorHandle,
};
use crate::{
    block_processing::BlockProcessorHandle,
    utils::{AsyncRuntimeHandle, ThreadPoolHandle},
    NetworkParamsDto, NodeFlagsHandle, StatHandle,
};
use rsnano_node::{
    bootstrap::{BulkPullClient, BulkPullClientExt},
    NetworkParams,
};
use std::sync::Arc;

pub struct BulkPullClientHandle(Arc<BulkPullClient>);

#[no_mangle]
pub extern "C" fn rsn_bulk_pull_client_create(
    network_params: &NetworkParamsDto,
    flags: &NodeFlagsHandle,
    stats: &StatHandle,
    block_processor: &BlockProcessorHandle,
    connection: &BootstrapClientHandle,
    attempt: &BootstrapAttemptHandle,
    workers: &ThreadPoolHandle,
    async_rt: &AsyncRuntimeHandle,
    connections: &BootstrapConnectionsHandle,
    bootstrap_init: &BootstrapInitiatorHandle,
    pull: &PullInfoDto,
) -> *mut BulkPullClientHandle {
    let network_params = NetworkParams::try_from(network_params).unwrap();
    let flags = flags.lock().unwrap().clone();
    Box::into_raw(Box::new(BulkPullClientHandle(Arc::new(
        BulkPullClient::new(
            network_params,
            flags,
            Arc::clone(stats),
            Arc::clone(block_processor),
            Arc::clone(connection),
            Arc::clone(attempt),
            Arc::clone(workers),
            Arc::clone(async_rt),
            Arc::clone(connections),
            Arc::clone(bootstrap_init),
            pull.into(),
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_client_destroy(handle: *mut BulkPullClientHandle) {
    drop(Box::from_raw(handle));
}
