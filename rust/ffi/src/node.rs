use crate::{
    fill_node_config_dto, to_rust_string, utils::AsyncRuntimeHandle, NetworkParamsDto,
    NodeConfigDto, StatHandle,
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
) -> *mut NodeHandle {
    let path = to_rust_string(path);
    Box::into_raw(Box::new(NodeHandle(Arc::new(Node::new(
        Arc::clone(async_rt),
        path,
        config.try_into().unwrap(),
        params.try_into().unwrap(),
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
