use crate::{
    bootstrap::{
        BootstrapMessageVisitor, BootstrapServer, BootstrapServerExt, BootstrapServerObserver,
        HandshakeMessageVisitor, RealtimeMessageVisitor, RequestResponseVisitorFactory,
    },
    ffi::{
        copy_account_bytes,
        io_context::{FfiIoContext, IoContextHandle},
        messages::FfiMessageVisitor,
        network::{EndpointDto, SocketHandle, TcpMessageManagerHandle},
        thread_pool::FfiThreadPool,
        voting::VoteUniquerHandle,
        BlockUniquerHandle, LoggerHandle, LoggerMT, NetworkFilterHandle, NetworkParamsDto,
        NodeConfigDto, StatHandle, VoidPointerCallback,
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
    pub block_uniquer: *mut BlockUniquerHandle,
    pub vote_uniquer: *mut VoteUniquerHandle,
    pub tcp_message_manager: *mut TcpMessageManagerHandle,
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
    let block_uniquer = Arc::clone(&*params.block_uniquer);
    let vote_uniquer = Arc::clone(&*params.vote_uniquer);
    let tcp_message_manager = Arc::clone(&*params.tcp_message_manager);
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
        block_uniquer,
        vote_uniquer,
        tcp_message_manager,
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
pub unsafe extern "C" fn rsn_bootstrap_server_to_realtime_connection(
    handle: *mut BootstrapServerHandle,
    node_id: *const u8,
) -> bool {
    let node_id = Account::from(node_id);
    (*handle).to_realtime_connection(&node_id)
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
pub unsafe extern "C" fn rsn_bootstrap_server_start(handle: *mut BootstrapServerHandle) {
    (*handle).start();
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
pub unsafe extern "C" fn rsn_bootstrap_server_telemetry_cutoff_exceeded(
    handle: *mut BootstrapServerHandle,
) -> bool {
    (*handle).is_telemetry_cutoff_exceeded()
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

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_socket(
    handle: *mut BootstrapServerHandle,
) -> *mut SocketHandle {
    SocketHandle::new((*handle).socket.clone())
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

type BootstrapServerTimeoutCallback = unsafe extern "C" fn(*mut c_void, usize);
type BootstrapServerExitedCallback =
    unsafe extern "C" fn(*mut c_void, u8, usize, *const EndpointDto);
type BootstrapServerBootstrapCountCallback = unsafe extern "C" fn(*mut c_void) -> usize;
type BootstrapServerIncBootstrapCountCallback = unsafe extern "C" fn(*mut c_void);

static mut DESTROY_OBSERVER_CALLBACK: Option<VoidPointerCallback> = None;
static mut TIMEOUT_CALLBACK: Option<BootstrapServerTimeoutCallback> = None;
static mut EXITED_CALLBACK: Option<BootstrapServerExitedCallback> = None;
static mut BOOTSTRAP_COUNT_CALLBACK: Option<BootstrapServerBootstrapCountCallback> = None;
static mut INC_BOOTSTRAP_COUNT_CALLBACK: Option<BootstrapServerIncBootstrapCountCallback> = None;
static mut INC_REALTIME_COUNT_CALLBACK: Option<BootstrapServerIncBootstrapCountCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_observer_destroy(f: VoidPointerCallback) {
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

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_observer_inc_realtime_count(
    f: BootstrapServerIncBootstrapCountCallback,
) {
    INC_REALTIME_COUNT_CALLBACK = Some(f);
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

    fn inc_realtime_count(&self) {
        unsafe {
            INC_REALTIME_COUNT_CALLBACK.expect("INC_REALTIME_COUNT_CALLBACK missing")(self.handle)
        }
    }
}

static mut DESTROY_VISITOR_FACTORY: Option<VoidPointerCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_request_response_visitor_factory_destroy(
    f: VoidPointerCallback,
) {
    DESTROY_VISITOR_FACTORY = Some(f);
}

/// first arg is a `shared_ptr<request_response_visitor_factory> *`
/// returns a `shared_ptr<message_visitor> *`
pub type RequestResponseVisitorFactoryCreateCallback =
    unsafe extern "C" fn(*mut c_void, *mut BootstrapServerHandle) -> *mut c_void;
static mut HANDSHAKE_VISITOR: Option<RequestResponseVisitorFactoryCreateCallback> = None;
static mut BOOTSTRAP_VISITOR: Option<RequestResponseVisitorFactoryCreateCallback> = None;
static mut REALTIME_VISITOR: Option<RequestResponseVisitorFactoryCreateCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_request_response_visitor_factory_handshake_visitor(
    f: RequestResponseVisitorFactoryCreateCallback,
) {
    HANDSHAKE_VISITOR = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_request_response_visitor_factory_bootstrap_visitor(
    f: RequestResponseVisitorFactoryCreateCallback,
) {
    BOOTSTRAP_VISITOR = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_request_response_visitor_factory_realtime_visitor(
    f: RequestResponseVisitorFactoryCreateCallback,
) {
    REALTIME_VISITOR = Some(f);
}

pub struct FfiRequestResponseVisitorFactory {
    handle: *mut c_void,
}

impl FfiRequestResponseVisitorFactory {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }

    fn create_visitor(
        &self,
        callback: Option<RequestResponseVisitorFactoryCreateCallback>,
        server: Arc<BootstrapServer>,
    ) -> Box<FfiMessageVisitor> {
        let visitor_handle = unsafe {
            callback.expect("RequestResponseVisitorFactory callbacks missing")(
                self.handle,
                BootstrapServerHandle::new(server),
            )
        };
        Box::new(FfiMessageVisitor::new(visitor_handle))
    }
}

impl Drop for FfiRequestResponseVisitorFactory {
    fn drop(&mut self) {
        unsafe { DESTROY_VISITOR_FACTORY.expect("DESTROY_VISITOR_FACTORY missing")(self.handle) }
    }
}

impl RequestResponseVisitorFactory for FfiRequestResponseVisitorFactory {
    fn handle(&self) -> *mut c_void {
        self.handle
    }

    fn handshake_visitor(&self, server: Arc<BootstrapServer>) -> Box<dyn HandshakeMessageVisitor> {
        unsafe { self.create_visitor(HANDSHAKE_VISITOR, server) }
    }

    fn realtime_visitor(&self, server: Arc<BootstrapServer>) -> Box<dyn RealtimeMessageVisitor> {
        unsafe { self.create_visitor(REALTIME_VISITOR, server) }
    }

    fn bootstrap_visitor(&self, server: Arc<BootstrapServer>) -> Box<dyn BootstrapMessageVisitor> {
        unsafe { self.create_visitor(BOOTSTRAP_VISITOR, server) }
    }
}
