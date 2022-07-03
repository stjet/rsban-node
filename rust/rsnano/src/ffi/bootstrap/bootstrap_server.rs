use crate::{
    bootstrap::BootstrapServer,
    ffi::{messages::MessageHandle, transport::SocketHandle, LoggerMT, NodeConfigDto},
    messages::Message,
    NodeConfig,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    ffi::c_void,
    rc::Rc,
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

pub struct BootstrapServerLockHandle(
    Rc<RefCell<Option<MutexGuard<'static, VecDeque<Option<Box<dyn Message>>>>>>>,
);

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_lock(
    handle: *mut BootstrapServerHandle,
) -> *mut BootstrapServerLockHandle {
    let guard = (*handle).0.queue.lock().unwrap();
    Box::into_raw(Box::new(BootstrapServerLockHandle(Rc::new(RefCell::new(
        Some(std::mem::transmute::<
            MutexGuard<VecDeque<Option<Box<dyn Message>>>>,
            MutexGuard<'static, VecDeque<Option<Box<dyn Message>>>>,
        >(guard)),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_lock_clone(
    handle: *mut BootstrapServerLockHandle,
) -> *mut BootstrapServerLockHandle {
    Box::into_raw(Box::new(BootstrapServerLockHandle((*handle).0.clone())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_unlock(lock_handle: *mut BootstrapServerLockHandle) {
    let mut inner = (*lock_handle).0.borrow_mut();
    *inner = None;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_relock(
    server_handle: *mut BootstrapServerHandle,
    lock_handle: *mut BootstrapServerLockHandle,
) {
    let guard = (*server_handle).0.queue.lock().unwrap();
    let mut inner = (*lock_handle).0.borrow_mut();
    *inner = Some(std::mem::transmute::<
        MutexGuard<VecDeque<Option<Box<dyn Message>>>>,
        MutexGuard<'static, VecDeque<Option<Box<dyn Message>>>>,
    >(guard));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_lock_destroy(handle: *mut BootstrapServerLockHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_release_front_request(
    handle: *mut BootstrapServerLockHandle,
) -> *mut MessageHandle {
    let mut requests = (*handle).0.borrow_mut();
    if let Some(r) = requests.as_mut() {
        if let Some(req) = r.front_mut() {
            if let Some(msg) = req.take() {
                return MessageHandle::new(msg);
            }
        }
    }

    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_queue_empty(
    handle: *mut BootstrapServerLockHandle,
) -> bool {
    let requests = (*handle).0.borrow();
    if let Some(r) = requests.as_ref() {
        r.is_empty()
    } else {
        true
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_requests_front(
    handle: *mut BootstrapServerLockHandle,
) -> *mut MessageHandle {
    let requests = (*handle).0.borrow();
    if let Some(r) = requests.as_ref() {
        if let Some(req) = r.front() {
            if let Some(msg) = req {
                return MessageHandle::new(msg.clone_box());
            }
        }
    }

    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_requests_pop(handle: *mut BootstrapServerLockHandle) {
    let mut requests = (*handle).0.borrow_mut();
    if let Some(r) = requests.as_mut() {
        r.pop_front();
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_requests_push(
    handle: *mut BootstrapServerLockHandle,
    msg: *mut MessageHandle,
) {
    let mut requests = (*handle).0.borrow_mut();
    if let Some(r) = requests.as_mut() {
        if msg.is_null(){
            r.push_back(None)
        }
        else{
            r.push_back(Some((*msg).clone_box()))
        }
    }
}
