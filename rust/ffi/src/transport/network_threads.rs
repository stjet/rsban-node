use super::{SynCookiesHandle, TcpChannelsHandle};
use crate::{NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle};
use rsnano_node::transport::NetworkThreads;
use std::sync::Arc;

pub struct NetworkThreadsHandle(NetworkThreads);

#[no_mangle]
pub extern "C" fn rsn_network_threads_create(
    channels: &TcpChannelsHandle,
    config: &NodeConfigDto,
    flags: &NodeFlagsHandle,
    network_params: &NetworkParamsDto,
    stats: &StatHandle,
    syn_cookies: &SynCookiesHandle,
) -> *mut NetworkThreadsHandle {
    Box::into_raw(Box::new(NetworkThreadsHandle(NetworkThreads::new(
        Arc::clone(channels),
        config.try_into().unwrap(),
        flags.lock().unwrap().clone(),
        network_params.try_into().unwrap(),
        Arc::clone(stats),
        Arc::clone(&syn_cookies),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_threads_destroy(handle: *mut NetworkThreadsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_network_threads_start(handle: &mut NetworkThreadsHandle) {
    handle.0.start();
}

#[no_mangle]
pub extern "C" fn rsn_network_threads_stop(handle: &mut NetworkThreadsHandle) {
    handle.0.stop();
}
