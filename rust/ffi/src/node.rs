use crate::{
    fill_node_config_dto,
    ledger::datastore::lmdb::LmdbStoreHandle,
    to_rust_string,
    utils::{AsyncRuntimeHandle, ThreadPoolHandle},
    work::{DistributedWorkFactoryHandle, WorkPoolHandle},
    NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle,
};
use rsnano_node::node::Node;
use std::{ffi::c_char, sync::Arc};

pub struct NodeHandle(Arc<Node>);

#[no_mangle]
pub unsafe extern "C" fn rsn_node_create(
    path: *const c_char,
    async_rt: &AsyncRuntimeHandle,
    config: &NodeConfigDto,
    params: &NetworkParamsDto,
    flags: &NodeFlagsHandle,
    work: &WorkPoolHandle,
) -> *mut NodeHandle {
    let path = to_rust_string(path);
    Box::into_raw(Box::new(NodeHandle(Arc::new(Node::new(
        Arc::clone(async_rt),
        path,
        config.try_into().unwrap(),
        params.try_into().unwrap(),
        flags.lock().unwrap().clone(),
        Arc::clone(work),
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
