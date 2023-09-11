use crate::{
    core::BlockUniquerHandle,
    transport::{EndpointDto, NetworkFilterHandle, SocketHandle, TcpMessageManagerHandle},
    utils::{IoContextHandle, LoggerHandle, LoggerMT},
    voting::VoteUniquerHandle,
    NetworkParamsDto, NodeConfigDto, StatHandle, VoidPointerCallback,
};
use rsnano_core::{utils::Logger, Account};
use rsnano_node::{
    config::NodeConfig,
    transport::{SocketType, TcpServer, TcpServerExt, TcpServerObserver},
    NetworkParams,
};
use std::{
    ffi::c_void,
    net::SocketAddr,
    ops::Deref,
    sync::{Arc, Weak},
};

use super::request_response_visitor_factory::RequestResponseVisitorFactoryHandle;

pub struct TcpServerHandle(pub Arc<TcpServer>);

impl TcpServerHandle {
    pub fn new(server: Arc<TcpServer>) -> *mut TcpServerHandle {
        Box::into_raw(Box::new(TcpServerHandle(server)))
    }
}

impl Deref for TcpServerHandle {
    type Target = Arc<TcpServer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct BootstrapServerWeakHandle(Weak<TcpServer>);

#[repr(C)]
pub struct CreateTcpServerParams {
    pub socket: *mut SocketHandle,
    pub config: *const NodeConfigDto,
    pub logger: *mut LoggerHandle,
    pub observer: *mut c_void,
    pub publish_filter: *mut NetworkFilterHandle,
    pub io_ctx: *mut IoContextHandle,
    pub network: *const NetworkParamsDto,
    pub disable_bootstrap_listener: bool,
    pub connections_max: usize,
    pub stats: *mut StatHandle,
    pub disable_bootstrap_bulk_pull_server: bool,
    pub disable_tcp_realtime: bool,
    pub request_response_visitor_factory: *mut RequestResponseVisitorFactoryHandle,
    pub block_uniquer: *mut BlockUniquerHandle,
    pub vote_uniquer: *mut VoteUniquerHandle,
    pub tcp_message_manager: *mut TcpMessageManagerHandle,
    pub allow_bootstrap: bool,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_create(
    params: &CreateTcpServerParams,
) -> *mut TcpServerHandle {
    let socket = Arc::clone(&(*params.socket));
    let config = Arc::new(NodeConfig::try_from(&*params.config).unwrap());
    let logger: Arc<dyn Logger> = Arc::new(LoggerMT::new(Box::from_raw(params.logger)));
    let observer = Arc::new(FfiBootstrapServerObserver::new(params.observer));
    let publish_filter = Arc::clone(&*params.publish_filter);
    let network = Arc::new(NetworkParams::try_from(&*params.network).unwrap());
    let stats = Arc::clone(&(*params.stats));
    let visitor_factory = Arc::clone(&(*params.request_response_visitor_factory).0);
    let block_uniquer = Arc::clone(&*params.block_uniquer);
    let vote_uniquer = Arc::clone(&*params.vote_uniquer);
    let tcp_message_manager = Arc::clone(&*params.tcp_message_manager);
    let mut server = TcpServer::new(
        socket,
        config,
        logger,
        observer,
        publish_filter,
        network,
        stats,
        block_uniquer,
        vote_uniquer,
        tcp_message_manager,
        visitor_factory,
        params.allow_bootstrap,
    );
    server.disable_bootstrap_listener = params.disable_bootstrap_listener;
    server.connections_max = params.connections_max;
    server.disable_bootstrap_bulk_pull_server = params.disable_bootstrap_bulk_pull_server;
    server.disable_tcp_realtime = params.disable_tcp_realtime;
    TcpServerHandle::new(Arc::new(server))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_destroy(handle: *mut TcpServerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_unique_id(handle: *mut TcpServerHandle) -> usize {
    (*handle).unique_id()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_get_weak(
    handle: *mut TcpServerHandle,
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
) -> *mut TcpServerHandle {
    match (*handle).0.upgrade() {
        Some(i) => TcpServerHandle::new(i),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_start(handle: *mut TcpServerHandle) {
    (*handle).start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_stop(handle: *mut TcpServerHandle) {
    (*handle).stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_is_stopped(handle: *mut TcpServerHandle) -> bool {
    (*handle).is_stopped()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_remote_endpoint(
    handle: *mut TcpServerHandle,
    endpoint: *mut EndpointDto,
) {
    (*endpoint) = (*handle).remote_endpoint().into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_set_remote_node_id(
    handle: *mut TcpServerHandle,
    node_id: *const u8,
) {
    let mut lk = (*handle).remote_node_id.lock().unwrap();
    *lk = Account::from_ptr(node_id);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_socket(
    handle: *mut TcpServerHandle,
) -> *mut SocketHandle {
    SocketHandle::new((*handle).socket.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_timeout(handle: *mut TcpServerHandle) {
    (*handle).timeout();
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

unsafe impl Send for FfiBootstrapServerObserver {}
unsafe impl Sync for FfiBootstrapServerObserver {}

impl Drop for FfiBootstrapServerObserver {
    fn drop(&mut self) {
        unsafe {
            DESTROY_OBSERVER_CALLBACK.expect("DESTROY_OBSERVER_CALLBACK missing")(self.handle);
        }
    }
}

impl TcpServerObserver for FfiBootstrapServerObserver {
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
