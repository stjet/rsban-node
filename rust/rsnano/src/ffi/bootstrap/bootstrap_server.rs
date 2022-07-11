use crate::{
    bootstrap::{BootstrapServer, BootstrapServerExt, BootstrapServerObserver},
    ffi::{
        copy_account_bytes, fill_network_params_dto, fill_node_config_dto,
        io_context::{FfiIoContext, IoContextHandle},
        messages::MessageHandle,
        thread_pool::FfiThreadPool,
        transport::{EndpointDto, SocketHandle},
        DestroyCallback, LoggerHandle, LoggerMT, NetworkFilterHandle, NetworkParamsDto,
        NodeConfigDto, StatHandle,
    },
    messages::Message,
    transport::SocketType,
    utils::BufferHandle,
    Account, NetworkParams, NodeConfig,
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

#[repr(C)]
pub struct CreateBootstrapServerParams {
    pub socket: *mut SocketHandle,
    pub config: *const NodeConfigDto,
    pub logger: *mut LoggerHandle,
    pub observer: *mut c_void,
    pub publish_filter: *mut NetworkFilterHandle,
    pub workers: *mut c_void,
    pub io_ctx: *mut IoContextHandle,
    pub network: *const NetworkParamsDto,
    pub disable_bootstrap_listener: bool,
    pub connections_max: usize,
    pub stats: *mut StatHandle,
    pub disable_bootstrap_bulk_pull_server: bool,
    pub disable_tcp_realtime: bool,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_create(
    params: &CreateBootstrapServerParams,
) -> *mut BootstrapServerHandle {
    let socket = Arc::clone(&(*params.socket));
    let config = Arc::new(NodeConfig::try_from(&*params.config).unwrap());
    let logger = Arc::new(LoggerMT::new(Box::from_raw(params.logger)));
    let observer = Arc::new(FfiBootstrapServerObserver::new(params.observer));
    let publish_filter = Arc::clone(&*params.publish_filter);
    let workers = Arc::new(FfiThreadPool::new(params.workers));
    let io_ctx = Arc::new(FfiIoContext::new((*params.io_ctx).raw_handle()));
    let network = NetworkParams::try_from(&*params.network).unwrap();
    let stats = Arc::clone(&(*params.stats));
    let mut server = BootstrapServer::new(
        socket,
        config,
        logger,
        observer,
        publish_filter,
        workers,
        io_ctx,
        network,
        stats,
    );
    server.disable_bootstrap_listener = params.disable_bootstrap_listener;
    server.connections_max = params.connections_max;
    server.disable_bootstrap_bulk_pull_server = params.disable_bootstrap_bulk_pull_server;
    server.disable_tcp_realtime = params.disable_tcp_realtime;
    Box::into_raw(Box::new(BootstrapServerHandle(Arc::new(server))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_destroy(handle: *mut BootstrapServerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_unique_id(
    handle: *mut BootstrapServerHandle,
) -> usize {
    (*handle).0.unique_id()
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

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_remote_endpoint(
    handle: *mut BootstrapServerHandle,
    endpoint: *mut EndpointDto,
) {
    let ep: SocketAddr = (*handle).0.remote_endpoint.lock().unwrap().clone();
    (*endpoint) = ep.into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_set_remote_endpoint(
    handle: *mut BootstrapServerHandle,
    endpoint: *const EndpointDto,
) {
    let mut lk = (*handle).0.remote_endpoint.lock().unwrap();
    *lk = SocketAddr::from(&*endpoint);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_remote_node_id(
    handle: *mut BootstrapServerHandle,
    node_id: *mut u8,
) {
    let lk = (*handle).0.remote_node_id.lock().unwrap();
    copy_account_bytes(*lk, node_id);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_set_remote_node_id(
    handle: *mut BootstrapServerHandle,
    node_id: *const u8,
) {
    let mut lk = (*handle).0.remote_node_id.lock().unwrap();
    *lk = Account::from(node_id);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_make_bootstrap_connection(
    handle: *mut BootstrapServerHandle,
) -> bool {
    (*handle).0.make_bootstrap_connection()
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
pub unsafe extern "C" fn rsn_bootstrap_server_socket(
    handle: *mut BootstrapServerHandle,
) -> *mut SocketHandle {
    SocketHandle::new((*handle).0.socket.clone())
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

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_receive_buffer(
    handle: *mut BootstrapServerHandle,
) -> *mut BufferHandle {
    BufferHandle::new((*handle).0.receive_buffer.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_publish_filter(
    handle: *mut BootstrapServerHandle,
) -> *mut NetworkFilterHandle {
    NetworkFilterHandle::new(Arc::clone(&(*handle).0.publish_filter))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_workers(
    handle: *mut BootstrapServerHandle,
) -> *mut c_void {
    (*handle).0.workers.handle()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_io_ctx(
    handle: *mut BootstrapServerHandle,
) -> *mut IoContextHandle {
    IoContextHandle::new((*handle).0.io_ctx.raw_handle())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_cache_exceeded(
    handle: *mut BootstrapServerHandle,
) -> bool {
    (*handle).0.cache_exceeded()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_set_last_telemetry_req(
    handle: *mut BootstrapServerHandle,
) {
    (*handle).0.set_last_telemetry_req();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_timeout(handle: *mut BootstrapServerHandle) {
    (*handle).0.timeout();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_logger(
    handle: *mut BootstrapServerHandle,
) -> *mut c_void {
    (*handle).0.logger.handle()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_stats(
    handle: *mut BootstrapServerHandle,
) -> *mut StatHandle {
    StatHandle::new(&(*handle).0.stats)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_config(
    handle: *mut BootstrapServerHandle,
    config: *mut NodeConfigDto,
) {
    fill_node_config_dto(&mut *config, &(*handle).0.config);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_network(
    handle: *mut BootstrapServerHandle,
    dto: *mut NetworkParamsDto,
) {
    fill_network_params_dto(&mut *dto, &(*handle).0.network);
}
#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_disable_bootstrap_bulk_pull_server(
    handle: *mut BootstrapServerHandle,
) -> bool {
    (*handle).0.disable_bootstrap_bulk_pull_server
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_disable_tcp_realtime(
    handle: *mut BootstrapServerHandle,
) -> bool {
    (*handle).0.disable_tcp_realtime
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
    fn bootstrap_server_timeout(&self, unique_id: usize) {
        unsafe {
            TIMEOUT_CALLBACK.expect("TIMEOUT_CALLBACK missing")(self.handle, unique_id);
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
