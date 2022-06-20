use crate::{
    bootstrap::BootstrapServer,
    ffi::{transport::SocketHandle, LoggerMT, NodeConfigDto},
    NodeConfig,
};
use std::{ffi::c_void, sync::Arc};

pub struct BootstrapServerHandle(Arc<BootstrapServer>);

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_create(
    socket: *mut SocketHandle,
    config: *const NodeConfigDto,
    logger: *mut c_void,
) -> *mut BootstrapServerHandle {
    let socket = Arc::clone(&(*socket));
    let config = Arc::new(NodeConfig::try_from(&*config).unwrap());
    let logger = Arc::new(LoggerMT::new(logger));
    Box::into_raw(Box::new(BootstrapServerHandle(Arc::new(
        BootstrapServer::new(socket, config, logger),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_destroy(handle: *mut BootstrapServerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_inner_ptr(
    handle: *mut BootstrapServerHandle,
) -> usize {
    let ptr = Arc::as_ptr(&(*handle).0);
    ptr as usize
}
