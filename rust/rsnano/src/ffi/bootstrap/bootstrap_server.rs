use crate::{
    bootstrap::BootstrapServer,
    ffi::{transport::SocketHandle, LoggerMT, NodeConfigDto},
    messages::Message,
    NodeConfig,
};
use std::{
    collections::VecDeque,
    ffi::c_void,
    sync::{Arc, MutexGuard},
};

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

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_stop(handle: *mut BootstrapServerHandle) {
    (*handle).0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_is_stopped(
    handle: *mut BootstrapServerHandle,
) -> bool {
    (*handle).0.is_stopped()
}

pub struct BootstrapServerLockHandle(Option<MutexGuard<'static, VecDeque<Box<dyn Message>>>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_lock(
    handle: *mut BootstrapServerHandle,
) -> *mut BootstrapServerLockHandle {
    let guard = (*handle).0.queue.lock().unwrap();
    Box::into_raw(Box::new(BootstrapServerLockHandle(Some(
        std::mem::transmute::<
            MutexGuard<VecDeque<Box<dyn Message>>>,
            MutexGuard<'static, VecDeque<Box<dyn Message>>>,
        >(guard),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_unlock(lock_handle: *mut BootstrapServerLockHandle) {
    (*lock_handle).0 = None
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_relock(
    server_handle: *mut BootstrapServerHandle,
    lock_handle: *mut BootstrapServerLockHandle,
) {
    let guard = (*server_handle).0.queue.lock().unwrap();
    (*lock_handle).0 = Some(std::mem::transmute::<
        MutexGuard<VecDeque<Box<dyn Message>>>,
        MutexGuard<'static, VecDeque<Box<dyn Message>>>,
    >(guard))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_lock_destroy(handle: *mut BootstrapServerLockHandle) {
    drop(Box::from_raw(handle));
}
