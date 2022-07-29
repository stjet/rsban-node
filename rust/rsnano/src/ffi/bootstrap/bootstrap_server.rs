use crate::{
    bootstrap::{
        BootstrapRequestsLock, BootstrapServer, BootstrapServerExt, BootstrapServerObserver,
        RequestResponseVisitorFactory,
    },
    ffi::{
        copy_account_bytes,
        io_context::{FfiIoContext, IoContextHandle},
        messages::{FfiMessageVisitor, MessageHandle},
        network::{EndpointDto, SocketHandle},
        thread_pool::FfiThreadPool,
        DestroyCallback, LoggerHandle, LoggerMT, NetworkFilterHandle, NetworkParamsDto,
        NodeConfigDto, StatHandle,
    },
    network::SocketType,
    Account, NetworkParams, NodeConfig,
};
use std::{
    ffi::c_void,
    net::SocketAddr,
    ops::Deref,
    sync::{atomic::Ordering, Arc, Weak},
};

pub struct BootstrapServerHandle(Arc<BootstrapServer>);

impl BootstrapServerHandle {
    pub fn new(server: Arc<BootstrapServer>) -> *mut BootstrapServerHandle {
        Box::into_raw(Box::new(BootstrapServerHandle(server)))
    }
}

impl Deref for BootstrapServerHandle {
    type Target = Arc<BootstrapServer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct BootstrapServerWeakHandle(Weak<BootstrapServer>);

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
    pub request_response_visitor_factory: *mut c_void,
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
    let visitor_factory = Arc::new(FfiRequestResponseVisitorFactory::new(
        params.request_response_visitor_factory,
    ));
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
        visitor_factory,
    );
    server.disable_bootstrap_listener = params.disable_bootstrap_listener;
    server.connections_max = params.connections_max;
    server.disable_bootstrap_bulk_pull_server = params.disable_bootstrap_bulk_pull_server;
    server.disable_tcp_realtime = params.disable_tcp_realtime;
    BootstrapServerHandle::new(Arc::new(server))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_destroy(handle: *mut BootstrapServerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_unique_id(
    handle: *mut BootstrapServerHandle,
) -> usize {
    (*handle).unique_id()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_get_weak(
    handle: *mut BootstrapServerHandle,
) -> *mut BootstrapServerWeakHandle {
    Box::into_raw(Box::new(BootstrapServerWeakHandle(Arc::downgrade(
        &*handle,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_destroy_weak(handle: *mut BootstrapServerWeakHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_copy_weak(
    handle: *mut BootstrapServerWeakHandle,
) -> *mut BootstrapServerWeakHandle {
    if handle.is_null() {
        std::ptr::null_mut()
    } else {
        Box::into_raw(Box::new(BootstrapServerWeakHandle((*handle).0.clone())))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_lock_weak(
    handle: *mut BootstrapServerWeakHandle,
) -> *mut BootstrapServerHandle {
    match (*handle).0.upgrade() {
        Some(i) => BootstrapServerHandle::new(i),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_stop(handle: *mut BootstrapServerHandle) {
    (*handle).stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_is_stopped(
    handle: *mut BootstrapServerHandle,
) -> bool {
    (*handle).is_stopped()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_remote_endpoint(
    handle: *mut BootstrapServerHandle,
    endpoint: *mut EndpointDto,
) {
    let ep: SocketAddr = (*handle).remote_endpoint.lock().unwrap().clone();
    (*endpoint) = ep.into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_remote_node_id(
    handle: *mut BootstrapServerHandle,
    node_id: *mut u8,
) {
    let lk = (*handle).remote_node_id.lock().unwrap();
    copy_account_bytes(*lk, node_id);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_set_remote_node_id(
    handle: *mut BootstrapServerHandle,
    node_id: *const u8,
) {
    let mut lk = (*handle).remote_node_id.lock().unwrap();
    *lk = Account::from(node_id);
}

pub struct BootstrapServerLockHandle(BootstrapRequestsLock);

impl BootstrapServerLockHandle {
    pub fn new(guard: BootstrapRequestsLock) -> *mut Self {
        Box::into_raw(Box::new(BootstrapServerLockHandle(guard)))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_lock_clone(
    handle: *mut BootstrapServerLockHandle,
) -> *mut BootstrapServerLockHandle {
    Box::into_raw(Box::new(BootstrapServerLockHandle((*handle).0.clone())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_lock_destroy(handle: *mut BootstrapServerLockHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_release_front_request(
    handle: *mut BootstrapServerLockHandle,
) -> *mut MessageHandle {
    match (*handle).0.release_front_request() {
        Some(msg) => MessageHandle::new(msg),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_socket(
    handle: *mut BootstrapServerHandle,
) -> *mut SocketHandle {
    SocketHandle::new((*handle).socket.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_requests_empty(
    handle: *mut BootstrapServerHandle,
) -> bool {
    (*handle).requests_empty()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_push_request(
    handle: *mut BootstrapServerHandle,
    msg: *mut MessageHandle,
) {
    let msg = if msg.is_null() {
        None
    } else {
        Some((*msg).clone_box())
    };

    (*handle).push_request(msg);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_set_last_telemetry_req(
    handle: *mut BootstrapServerHandle,
) {
    (*handle).set_last_telemetry_req();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_timeout(handle: *mut BootstrapServerHandle) {
    (*handle).timeout();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_disable_bootstrap_bulk_pull_server(
    handle: *mut BootstrapServerHandle,
) -> bool {
    (*handle).disable_bootstrap_bulk_pull_server
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_handshake_query_received(
    handle: *mut BootstrapServerHandle,
) -> bool {
    (*handle).handshake_query_received.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_set_handshake_query_received(
    handle: *mut BootstrapServerHandle,
) {
    (*handle)
        .0
        .handshake_query_received
        .store(true, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_receive(handle: *mut BootstrapServerHandle) {
    (*handle).receive();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_finish_request_async(
    handle: *mut BootstrapServerHandle,
) {
    (*handle).finish_request_async();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_finish_request(handle: *mut BootstrapServerHandle) {
    (*handle).finish_request();
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

static mut DESTROY_VISITOR_FACTORY: Option<DestroyCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_request_response_visitor_factory_destroy(f: DestroyCallback) {
    DESTROY_VISITOR_FACTORY = Some(f);
}

/// first arg is a `shared_ptr<request_response_visitor_factory> *`
/// returns a `shared_ptr<message_visitor> *`
pub type RequestResponseVisitorFactoryCreateCallback = unsafe extern "C" fn(
    *mut c_void,
    *mut BootstrapServerHandle,
    *mut BootstrapServerLockHandle,
) -> *mut c_void;
static mut CREATE_VISITOR: Option<RequestResponseVisitorFactoryCreateCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_request_response_visitor_factory_create(
    f: RequestResponseVisitorFactoryCreateCallback,
) {
    CREATE_VISITOR = Some(f);
}

pub struct FfiRequestResponseVisitorFactory {
    handle: *mut c_void,
}

impl FfiRequestResponseVisitorFactory {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

impl Drop for FfiRequestResponseVisitorFactory {
    fn drop(&mut self) {
        unsafe { DESTROY_VISITOR_FACTORY.expect("DESTROY_VISITOR_FACTORY missing")(self.handle) }
    }
}

impl RequestResponseVisitorFactory for FfiRequestResponseVisitorFactory {
    fn create_visitor(
        &self,
        connection: &Arc<BootstrapServer>,
        requests_lock: &BootstrapRequestsLock,
    ) -> Box<dyn crate::messages::MessageVisitor> {
        let visitor_handle = unsafe {
            CREATE_VISITOR.expect("CREATE_VISITOR missing")(
                self.handle,
                BootstrapServerHandle::new(connection.clone()),
                BootstrapServerLockHandle::new(requests_lock.clone()),
            )
        };
        Box::new(FfiMessageVisitor::new(visitor_handle))
    }

    fn handle(&self) -> *mut c_void {
        self.handle
    }
}
