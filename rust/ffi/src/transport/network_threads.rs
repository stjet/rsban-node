use super::{SynCookiesHandle, TcpChannelsHandle};
use crate::{NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle};
use rsnano_node::transport::NetworkThreads;
use std::{borrow::BorrowMut, sync::Arc};

pub struct NetworkThreadsHandle(pub Arc<NetworkThreads>);

#[no_mangle]
pub extern "C" fn rsn_network_threads_create(
    channels: &TcpChannelsHandle,
    config: &NodeConfigDto,
    flags: &NodeFlagsHandle,
    network_params: &NetworkParamsDto,
    stats: &StatHandle,
    syn_cookies: &SynCookiesHandle,
) -> *mut NetworkThreadsHandle {
    Box::into_raw(Box::new(NetworkThreadsHandle(Arc::new(
        NetworkThreads::new(
            Arc::clone(channels),
            config.try_into().unwrap(),
            flags.lock().unwrap().clone(),
            network_params.try_into().unwrap(),
            Arc::clone(stats),
            Arc::clone(&syn_cookies),
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_threads_destroy(handle: *mut NetworkThreadsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_threads_start(handle: &mut NetworkThreadsHandle) {
    let mut_threads = Arc::as_ptr(&handle.0) as *mut NetworkThreads;
    (*mut_threads).start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_threads_stop(handle: &mut NetworkThreadsHandle) {
    let mut_threads = Arc::as_ptr(&handle.0) as *mut NetworkThreads;
    (*mut_threads).stop();
}
