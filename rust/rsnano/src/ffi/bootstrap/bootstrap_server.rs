use crate::{
    bootstrap::{BootstrapServer, BootstrapServerObserver},
    ffi::{
        messages::MessageHandle,
        transport::{EndpointDto, SocketHandle},
        DestroyCallback, LoggerMT, NodeConfigDto,
    },
    messages::Message,
    transport::SocketType,
    NodeConfig,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    ffi::c_void,
    net::SocketAddr,
    rc::Rc,
    sync::{Arc, MutexGuard},
};

pub struct BootstrapServerHandle(Arc<BootstrapServer>);

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_create(
    socket: *mut SocketHandle,
    config: *const NodeConfigDto,
    logger: *mut c_void,
    observer: *mut c_void,
) -> *mut BootstrapServerHandle {
    let socket = Arc::clone(&(*socket));
    let config = Arc::new(NodeConfig::try_from(&*config).unwrap());
    let logger = Arc::new(LoggerMT::new(logger));
    let observer = Arc::new(FfiBootstrapServerObserver::new(observer));
    Box::into_raw(Box::new(BootstrapServerHandle(Arc::new(
        BootstrapServer::new(socket, config, logger, observer),
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
        if msg.is_null() {
            r.push_back(None)
        } else {
            r.push_back(Some((*msg).clone_box()))
        }
    }
}

type BootstrapServerTimeoutCallback = unsafe extern "C" fn(*mut c_void, usize);
type BootstrapServerExitedCallback =
    unsafe extern "C" fn(*mut c_void, u8, usize, *const EndpointDto);
type BootstrapServerBootstrapCountCallback = unsafe extern "C" fn(*mut c_void) -> usize;
type BootstrapServerIncBootstrapCountCallback = unsafe extern "C" fn(*mut c_void);

static mut DESTROY_OBSERVER_CALLBACK: Option<DestroyCallback> = None;
static mut TIMEOUT_CALLBACK: Option<BootstrapServerTimeoutCallback> = None;
static mut EXITED_CALLBACK: Option<BootstrapServerExitedCallback> = None;
static mut BOOTSTRAP_COUNT_CALLBACK: Option<BootstrapServerBootstrapCountCallback> = None;
static mut INC_BOOTSTRAP_COUNT_CALLBACK: Option<BootstrapServerIncBootstrapCountCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_observer_destroy(f: DestroyCallback) {
    DESTROY_OBSERVER_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_observer_timeout(
    f: BootstrapServerTimeoutCallback,
) {
    TIMEOUT_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_observer_exited(f: BootstrapServerExitedCallback) {
    EXITED_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_observer_bootstrap_count(
    f: BootstrapServerBootstrapCountCallback,
) {
    BOOTSTRAP_COUNT_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_observer_inc_bootstrap_count(
    f: BootstrapServerIncBootstrapCountCallback,
) {
    INC_BOOTSTRAP_COUNT_CALLBACK = Some(f);
}

pub struct FfiBootstrapServerObserver {
    handle: *mut c_void,
}

impl FfiBootstrapServerObserver {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

impl Drop for FfiBootstrapServerObserver {
    fn drop(&mut self) {
        unsafe {
            DESTROY_OBSERVER_CALLBACK.expect("DESTROY_OBSERVER_CALLBACK missing")(self.handle);
        }
    }
}

impl BootstrapServerObserver for FfiBootstrapServerObserver {
    fn bootstrap_server_timeout(&self, inner_ptr: usize) {
        unsafe {
            TIMEOUT_CALLBACK.expect("TIMEOUT_CALLBACK missing")(self.handle, inner_ptr);
        }
    }

    fn boostrap_server_exited(
        &self,
        socket_type: SocketType,
        inner_ptr: usize,
        endpoint: SocketAddr,
    ) {
        let endpoint_dto = EndpointDto::from(&endpoint);
        unsafe {
            EXITED_CALLBACK.expect("EXITED_CALLBACK missing")(
                self.handle,
                socket_type as u8,
                inner_ptr,
                &endpoint_dto,
            );
        }
    }

    fn get_bootstrap_count(&self) -> usize {
        unsafe { BOOTSTRAP_COUNT_CALLBACK.expect("BOOTSTRAP_COUNT_CALLBACK missing")(self.handle) }
    }

    fn inc_bootstrap_count(&self) {
        unsafe {
            INC_BOOTSTRAP_COUNT_CALLBACK.expect("INC_BOOTSTRAP_COUNT_CALLBACK missing")(self.handle)
        }
    }
}
