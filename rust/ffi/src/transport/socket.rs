use num::FromPrimitive;

use rsnano_node::{
    stats::SocketStats,
    transport::{
        CompositeSocketObserver, Socket, SocketBuilder, SocketExtensions, SocketObserver,
        SocketType, TcpSocketFacade, TcpSocketFacadeFactory, TokioSocketFacade, WriteCallback,
    },
    utils::{BufferWrapper, ErrorCode},
};
use std::{
    ffi::c_void,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6},
    ops::Deref,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

use crate::{
    utils::{
        is_tokio_enabled, AsyncRuntimeHandle, DispatchCallback, LoggerHandle, LoggerMT,
        ThreadPoolHandle, VoidFnCallbackHandle,
    },
    ErrorCodeDto, StatHandle, StringDto, VoidPointerCallback,
};

pub struct BufferHandle(Arc<Mutex<Vec<u8>>>);

impl BufferHandle {
    pub fn new(buf: Arc<Mutex<Vec<u8>>>) -> *mut BufferHandle {
        Box::into_raw(Box::new(BufferHandle(buf)))
    }
}

impl Deref for BufferHandle {
    type Target = Arc<Mutex<Vec<u8>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_buffer_create(len: usize) -> *mut BufferHandle {
    Box::into_raw(Box::new(BufferHandle(Arc::new(Mutex::new(vec![0; len])))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_buffer_destroy(handle: *mut BufferHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_buffer_data(handle: *mut BufferHandle) -> *mut u8 {
    let ptr = (*handle).0.lock().unwrap().as_ptr();
    std::mem::transmute::<*const u8, *mut u8>(ptr)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_buffer_len(handle: *mut BufferHandle) -> usize {
    (*handle).0.lock().unwrap().len()
}

pub struct SocketHandle(pub Arc<Socket>);
pub struct SocketWeakHandle(Weak<Socket>);

impl SocketHandle {
    pub fn new(socket: Arc<Socket>) -> *mut SocketHandle {
        Box::into_raw(Box::new(SocketHandle(socket)))
    }
}

impl Deref for SocketHandle {
    type Target = Arc<Socket>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_create(
    endpoint_type: u8,
    tcp_facade_handle: *mut c_void,
    stats_handle: *mut StatHandle,
    thread_pool: &ThreadPoolHandle,
    default_timeout_s: u64,
    silent_connection_tolerance_time_s: u64,
    idle_timeout_s: u64,
    network_timeout_logging: bool,
    logger: *mut LoggerHandle,
    callback_handler: *mut c_void,
    max_write_queue_len: usize,
    async_rt: &AsyncRuntimeHandle,
) -> *mut SocketHandle {
    let endpoint_type = FromPrimitive::from_u8(endpoint_type).unwrap();
    let mut tcp_facade: Arc<dyn TcpSocketFacade> =
        Arc::new(FfiTcpSocketFacade::new(tcp_facade_handle));
    if is_tokio_enabled() {
        tcp_facade = Arc::new(TokioSocketFacade::new(Arc::clone(&async_rt.0.tokio)));
    }
    let thread_pool = thread_pool.0.clone();
    let logger = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let stats = (*stats_handle).deref().clone();

    let socket_stats = Arc::new(SocketStats::new(stats, logger, network_timeout_logging));
    let ffi_observer = Arc::new(SocketFfiObserver::new(callback_handler));

    let socket = SocketBuilder::endpoint_type(endpoint_type, tcp_facade, thread_pool)
        .default_timeout(Duration::from_secs(default_timeout_s))
        .silent_connection_tolerance_time(Duration::from_secs(silent_connection_tolerance_time_s))
        .idle_timeout(Duration::from_secs(idle_timeout_s))
        .observer(Arc::new(CompositeSocketObserver::new(vec![
            socket_stats,
            ffi_observer,
        ])))
        .max_write_queue_len(max_write_queue_len)
        .build();

    SocketHandle::new(socket)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_destroy(handle: *mut SocketHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_start(handle: *mut SocketHandle) {
    (*handle).start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_inner_ptr(handle: *mut SocketHandle) -> *const c_void {
    let p = Arc::as_ptr(&(*handle).0);
    p as *const c_void
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_to_weak_handle(
    handle: *mut SocketHandle,
) -> *mut SocketWeakHandle {
    Box::into_raw(Box::new(SocketWeakHandle(Arc::downgrade(&(*handle).0))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_weak_socket_destroy(handle: *mut SocketWeakHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_weak_socket_to_socket(
    handle: *mut SocketWeakHandle,
) -> *mut SocketHandle {
    match (*handle).0.upgrade() {
        Some(socket) => SocketHandle::new(socket),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_weak_socket_expired(handle: *mut SocketWeakHandle) -> bool {
    (*handle).0.strong_count() == 0
}

type SocketConnectCallback = unsafe extern "C" fn(*mut c_void, *const ErrorCodeDto);
pub type SocketDestroyContext = unsafe extern "C" fn(*mut c_void);

struct ConnectCallbackWrapper {
    callback: SocketConnectCallback,
    destory_context: SocketDestroyContext,
    context: *mut c_void,
}

unsafe impl Send for ConnectCallbackWrapper {}

impl ConnectCallbackWrapper {
    fn new(
        callback: SocketConnectCallback,
        destory_context: SocketDestroyContext,
        context: *mut c_void,
    ) -> Self {
        Self {
            callback,
            destory_context,
            context,
        }
    }
    fn execute(&self, ec: ErrorCode) {
        let ec_dto = ErrorCodeDto::from(&ec);
        unsafe { (self.callback)(self.context, &ec_dto) };
    }
}

impl Drop for ConnectCallbackWrapper {
    fn drop(&mut self) {
        unsafe { (self.destory_context)(self.context) };
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_async_connect(
    handle: *mut SocketHandle,
    endpoint: *const EndpointDto,
    callback: SocketConnectCallback,
    destroy_context: SocketDestroyContext,
    context: *mut c_void,
) {
    let cb_wrapper = ConnectCallbackWrapper::new(callback, destroy_context, context);
    let cb = Box::new(move |ec| {
        cb_wrapper.execute(ec);
    });
    (*handle).async_connect((&*endpoint).into(), cb);
}

pub struct ReadCallbackWrapper {
    callback: SocketReadCallback,
    destory_context: SocketDestroyContext,
    context: *mut c_void,
}

impl ReadCallbackWrapper {
    pub fn new(
        callback: SocketReadCallback,
        destory_context: SocketDestroyContext,
        context: *mut c_void,
    ) -> Self {
        Self {
            callback,
            destory_context,
            context,
        }
    }

    pub fn execute(&self, ec: ErrorCode, size: usize) {
        let ec_dto = ErrorCodeDto::from(&ec);
        unsafe { (self.callback)(self.context, &ec_dto, size) };
    }
}

impl Drop for ReadCallbackWrapper {
    fn drop(&mut self) {
        unsafe { (self.destory_context)(self.context) };
    }
}

unsafe impl Send for ReadCallbackWrapper {}
unsafe impl Sync for ReadCallbackWrapper {}

pub type SocketReadCallback = unsafe extern "C" fn(*mut c_void, *const ErrorCodeDto, usize);

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_async_read(
    handle: *mut SocketHandle,
    buffer: *mut c_void,
    size: usize,
    callback: SocketReadCallback,
    destroy_context: SocketDestroyContext,
    context: *mut c_void,
) {
    let cb_wrapper = ReadCallbackWrapper::new(callback, destroy_context, context);
    let cb = Box::new(move |ec, size| {
        cb_wrapper.execute(ec, size);
    });
    let buffer_wrapper = Arc::new(FfiBufferWrapper::new(buffer));
    (*handle).async_read(buffer_wrapper, size, cb);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_async_read2(
    handle: *mut SocketHandle,
    buffer: *mut BufferHandle,
    size: usize,
    callback: SocketReadCallback,
    destroy_context: SocketDestroyContext,
    context: *mut c_void,
) {
    let cb_wrapper = ReadCallbackWrapper::new(callback, destroy_context, context);
    let cb = Box::new(move |ec, size| {
        cb_wrapper.execute(ec, size);
    });
    (*handle).async_read2(Arc::clone(&(*buffer)), size, cb);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_async_write(
    handle: *mut SocketHandle,
    buffer: *const u8,
    buffer_len: usize,
    callback: SocketReadCallback,
    destroy_context: SocketDestroyContext,
    context: *mut c_void,
    traffic_type: u8,
) {
    let cb: Option<WriteCallback> = if !context.is_null() {
        let cb_wrapper = ReadCallbackWrapper::new(callback, destroy_context, context);
        Some(Box::new(move |ec, size| {
            cb_wrapper.execute(ec, size);
        }))
    } else {
        None
    };
    let buffer = std::slice::from_raw_parts(buffer, buffer_len);
    (*handle).async_write(
        &Arc::new(buffer.to_vec()),
        cb,
        FromPrimitive::from_u8(traffic_type).unwrap(),
    );
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_local_endpoint(
    handle: *mut SocketHandle,
    endpoint: *mut EndpointDto,
) {
    let ep = (*handle).local_endpoint();
    set_enpoint_dto(&ep, &mut (*endpoint))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_remote_endpoint(
    handle: *mut SocketHandle,
    endpoint: *const EndpointDto,
) {
    (*handle).set_remote(SocketAddr::from(&*endpoint))
}

fn set_enpoint_dto(endpoint: &SocketAddr, result: &mut EndpointDto) {
    result.port = endpoint.port();
    match endpoint {
        SocketAddr::V4(addr) => {
            result.v6 = false;
            result.bytes[..4].copy_from_slice(&addr.ip().octets());
        }
        SocketAddr::V6(addr) => {
            result.v6 = true;
            result.bytes.copy_from_slice(&addr.ip().octets());
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_get_remote(
    handle: *mut SocketHandle,
    result: *mut EndpointDto,
) {
    match (*handle).get_remote() {
        Some(ep) => {
            set_enpoint_dto(&ep, &mut *result);
        }
        None => {
            (*result).port = 0;
            (*result).v6 = false;
            (*result).bytes = [0; 16];
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_endpoint_type(handle: *mut SocketHandle) -> u8 {
    (*handle).endpoint_type() as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_max(handle: *mut SocketHandle, traffic_type: u8) -> bool {
    (*handle).max(FromPrimitive::from_u8(traffic_type).unwrap())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_full(handle: *mut SocketHandle, traffic_type: u8) -> bool {
    (*handle).full(FromPrimitive::from_u8(traffic_type).unwrap())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_silent_connection_tolerance_time(
    handle: *mut SocketHandle,
    time_s: u64,
) {
    (*handle).set_silent_connection_tolerance_time(time_s)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_timeout(handle: *mut SocketHandle, timeout_s: u64) {
    (*handle).set_timeout(Duration::from_secs(timeout_s));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_close(handle: *mut SocketHandle) {
    (*handle).close()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_close_internal(handle: *mut SocketHandle) {
    (*handle).close_internal();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_is_closed(handle: *mut SocketHandle) -> bool {
    (*handle).is_closed()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_has_timed_out(handle: *mut SocketHandle) -> bool {
    (*handle).has_timed_out()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_checkup(handle: *mut SocketHandle) {
    (*handle).ongoing_checkup();
}

pub struct AsyncConnectCallbackHandle(Option<Box<dyn FnOnce(ErrorCode)>>);

impl AsyncConnectCallbackHandle {
    pub fn new(callback: Box<dyn FnOnce(ErrorCode)>) -> Self {
        Self(Some(callback))
    }
}

type AsyncConnectCallback =
    unsafe extern "C" fn(*mut c_void, *const EndpointDto, *mut AsyncConnectCallbackHandle);

static mut ASYNC_CONNECT_CALLBACK: Option<AsyncConnectCallback> = None;

type RemoteEndpointCallback =
    unsafe extern "C" fn(*mut c_void, *mut EndpointDto, *mut ErrorCodeDto);

static mut REMOTE_ENDPOINT_CALLBACK: Option<RemoteEndpointCallback> = None;

static mut DISPATCH_CALLBACK: Option<DispatchCallback> = None;
static mut POST_CALLBACK: Option<DispatchCallback> = None;

type CloseSocketCallback = unsafe extern "C" fn(*mut c_void, *mut ErrorCodeDto);

static mut CLOSE_SOCKET_CALLBACK: Option<CloseSocketCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_async_connect(f: AsyncConnectCallback) {
    ASYNC_CONNECT_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_remote_endpoint(f: RemoteEndpointCallback) {
    REMOTE_ENDPOINT_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_dispatch(f: DispatchCallback) {
    DISPATCH_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_post(f: DispatchCallback) {
    POST_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_close(f: CloseSocketCallback) {
    CLOSE_SOCKET_CALLBACK = Some(f);
}

pub struct AsyncReadCallbackHandle(Option<Box<dyn FnOnce(ErrorCode, usize)>>);

type AsyncReadCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, usize, *mut AsyncReadCallbackHandle);

type AsyncRead2Callback =
    unsafe extern "C" fn(*mut c_void, *mut BufferHandle, usize, *mut AsyncReadCallbackHandle);

static mut ASYNC_READ_CALLBACK: Option<AsyncReadCallback> = None;
static mut ASYNC_READ2_CALLBACK: Option<AsyncRead2Callback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_async_read(f: AsyncReadCallback) {
    ASYNC_READ_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_async_read2(f: AsyncRead2Callback) {
    ASYNC_READ2_CALLBACK = Some(f);
}

pub struct AsyncWriteCallbackHandle(Option<Box<dyn FnOnce(ErrorCode, usize)>>);

impl AsyncWriteCallbackHandle {
    pub fn new(callback: Box<dyn FnOnce(ErrorCode, usize)>) -> Self {
        Self(Some(callback))
    }
}

type AsyncWriteCallback =
    unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut AsyncWriteCallbackHandle);

static mut ASYNC_WRITE_CALLBACK: Option<AsyncWriteCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_async_write(f: AsyncWriteCallback) {
    ASYNC_WRITE_CALLBACK = Some(f);
}

pub struct AsyncAcceptCallbackHandle(Option<Box<dyn FnOnce(SocketAddr, ErrorCode)>>);

type AsyncAcceptCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *mut AsyncAcceptCallbackHandle);

static mut ASYNC_ACCEPT_CALLBACK: Option<AsyncAcceptCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_async_accept(f: AsyncAcceptCallback) {
    ASYNC_ACCEPT_CALLBACK = Some(f);
}

static mut TCP_FACADE_DESTROY_CALLBACK: Option<VoidPointerCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_destroy(f: VoidPointerCallback) {
    TCP_FACADE_DESTROY_CALLBACK = Some(f);
}

type TcpSocketListeningPortCallback = unsafe extern "C" fn(*mut c_void) -> u16;
static mut TCP_SOCKET_LISTENING_PORT: Option<TcpSocketListeningPortCallback> = None;

type TcpSocketOpenCallback =
    unsafe extern "C" fn(*mut c_void, *const EndpointDto, *mut ErrorCodeDto);
static mut TCP_SOCKET_OPEN: Option<TcpSocketOpenCallback> = None;

type SocketLocalEndpointCallback = unsafe extern "C" fn(*mut c_void, *mut EndpointDto);
static mut LOCAL_ENDPOINT_CALLBACK: Option<SocketLocalEndpointCallback> = None;

type SocketIsOpenCallback = unsafe extern "C" fn(*mut c_void) -> bool;
static mut SOCKET_IS_OPEN_CALLBACK: Option<SocketIsOpenCallback> = None;

type SocketConnectedCallback = unsafe extern "C" fn(*mut c_void, *mut SocketHandle);
static mut SOCKET_CONNECTED_CALLBACK: Option<SocketConnectedCallback> = None;
static mut SOCKET_ACCEPTED_CALLBACK: Option<SocketConnectedCallback> = None;
static mut DELETE_TCP_SOCKET_CALLBACK: Option<VoidPointerCallback> = None;

pub type CreateTcpSocketCallback = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
static mut CREATE_TCP_SOCKET_CALLBACK: Option<CreateTcpSocketCallback> = None;
static mut DESTROY_TCP_SOCKET_FACADE_FACTORY_CALLBACK: Option<VoidPointerCallback> = None;
static mut SOCKET_CLOSE_ACCEPTOR_CALLBACK: Option<VoidPointerCallback> = None;
static mut IS_ACCEPTOR_OPEN: Option<SocketIsOpenCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_create_tcp_socket(f: CreateTcpSocketCallback) {
    CREATE_TCP_SOCKET_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_destroy_tcp_socket_facade_factory(f: VoidPointerCallback) {
    DESTROY_TCP_SOCKET_FACADE_FACTORY_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_listening_port(f: TcpSocketListeningPortCallback) {
    TCP_SOCKET_LISTENING_PORT = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_open(f: TcpSocketOpenCallback) {
    TCP_SOCKET_OPEN = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_local_endpoint(f: SocketLocalEndpointCallback) {
    LOCAL_ENDPOINT_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_is_acceptor_open(f: SocketIsOpenCallback) {
    IS_ACCEPTOR_OPEN = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_is_open(f: SocketIsOpenCallback) {
    SOCKET_IS_OPEN_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_connected(f: SocketConnectedCallback) {
    SOCKET_CONNECTED_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_accepted(f: SocketConnectedCallback) {
    SOCKET_ACCEPTED_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_delete_tcp_socket_callback(f: VoidPointerCallback) {
    DELETE_TCP_SOCKET_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_socket_close_acceptor_callback(f: VoidPointerCallback) {
    SOCKET_CLOSE_ACCEPTOR_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_connect_callback_execute(
    callback: *mut AsyncConnectCallbackHandle,
    ec: *const ErrorCodeDto,
) {
    let error_code = ErrorCode::from(&*ec);
    if let Some(cb) = (*callback).0.take() {
        cb(error_code);
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_connect_callback_destroy(
    callback: *mut AsyncConnectCallbackHandle,
) {
    drop(Box::from_raw(callback))
}

#[no_mangle]
pub extern "C" fn rsn_async_accept_callback_execute(
    callback: &mut AsyncAcceptCallbackHandle,
    ec: &ErrorCodeDto,
    remote_endpoint: &EndpointDto,
) {
    let error_code = ErrorCode::from(&*ec);
    let remote_endpoint = SocketAddr::from(remote_endpoint);
    if let Some(cb) = (*callback).0.take() {
        cb(remote_endpoint, error_code);
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_accept_callback_destroy(
    callback: *mut AsyncAcceptCallbackHandle,
) {
    drop(Box::from_raw(callback))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_default_timeout_value(
    handle: *mut SocketHandle,
    timeout_s: u64,
) {
    (*handle).set_default_timeout_value(timeout_s)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_type(handle: *mut SocketHandle) -> u8 {
    (*handle).socket_type() as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_type(handle: *mut SocketHandle, socket_type: u8) {
    (*handle).set_socket_type(SocketType::from_u8(socket_type).unwrap());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_is_bootstrap_connection(handle: *mut SocketHandle) -> bool {
    (*handle).is_bootstrap_connection()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_default_timeout_value(handle: *mut SocketHandle) -> u64 {
    (*handle).default_timeout_value()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_is_alive(handle: *mut SocketHandle) -> bool {
    (*handle).is_alive()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_type_to_string(socket_type: u8, result: *mut StringDto) {
    *result = StringDto::from(SocketType::from_u8(socket_type).unwrap().as_str())
}

pub struct FfiTcpSocketFacadeFactory(pub *mut c_void);

unsafe impl Send for FfiTcpSocketFacadeFactory {}
unsafe impl Sync for FfiTcpSocketFacadeFactory {}

impl TcpSocketFacadeFactory for FfiTcpSocketFacadeFactory {
    fn create_tcp_socket(&self) -> Arc<dyn TcpSocketFacade> {
        let handle = unsafe {
            CREATE_TCP_SOCKET_CALLBACK.expect("CREATE_TCP_SOCKET_CALLBACK missing")(self.0)
        };
        Arc::new(FfiTcpSocketFacade::new(handle))
    }
}

impl Drop for FfiTcpSocketFacadeFactory {
    fn drop(&mut self) {
        unsafe {
            DESTROY_TCP_SOCKET_FACADE_FACTORY_CALLBACK
                .expect("DESTROY_TCP_SOCKET_FACADE_FACTORY_CALLBACK missing")(self.0)
        }
    }
}

pub struct FfiTcpSocketFacade {
    handle: *mut c_void,
}

impl FfiTcpSocketFacade {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

impl Drop for FfiTcpSocketFacade {
    fn drop(&mut self) {
        unsafe {
            TCP_FACADE_DESTROY_CALLBACK.expect("TCP_FACADE_DESTROY_CALLBACK missing")(self.handle)
        }
    }
}

impl TcpSocketFacade for FfiTcpSocketFacade {
    fn async_connect(&self, endpoint: SocketAddr, callback: Box<dyn FnOnce(ErrorCode) + Send>) {
        let endpoint_dto = EndpointDto::from(&endpoint);
        let callback_handle = Box::new(AsyncConnectCallbackHandle::new(callback));
        unsafe {
            match ASYNC_CONNECT_CALLBACK {
                Some(f) => f(self.handle, &endpoint_dto, Box::into_raw(callback_handle)),
                None => panic!("ASYNC_CONNECT_CALLBACK missing"),
            }
        }
    }

    fn async_read(
        &self,
        buffer: &Arc<dyn BufferWrapper>,
        len: usize,
        callback: Box<dyn FnOnce(ErrorCode, usize) + Send>,
    ) {
        let callback_handle = Box::into_raw(Box::new(AsyncReadCallbackHandle(Some(callback))));
        unsafe {
            ASYNC_READ_CALLBACK.expect("ASYNC_READ_CALLBACK missing")(
                self.handle,
                buffer.handle(),
                len,
                callback_handle,
            );
        }
    }

    fn async_read2(
        &self,
        buffer: &Arc<Mutex<Vec<u8>>>,
        len: usize,
        callback: Box<dyn FnOnce(ErrorCode, usize) + Send>,
    ) {
        let callback_handle = Box::into_raw(Box::new(AsyncReadCallbackHandle(Some(callback))));
        unsafe {
            ASYNC_READ2_CALLBACK.expect("ASYNC_READ2_CALLBACK missing")(
                self.handle,
                BufferHandle::new(buffer.clone()),
                len,
                callback_handle,
            );
        }
    }

    fn async_write(
        &self,
        buffer: &Arc<Vec<u8>>,
        callback: Box<dyn FnOnce(ErrorCode, usize) + Send>,
    ) {
        let callback_handle = Box::into_raw(Box::new(AsyncWriteCallbackHandle::new(callback)));
        unsafe {
            ASYNC_WRITE_CALLBACK.expect("ASYNC_WRITE_CALLBACK missing")(
                self.handle,
                buffer.as_ptr(),
                buffer.len(),
                callback_handle,
            );
        }
    }

    fn remote_endpoint(&self) -> Result<SocketAddr, ErrorCode> {
        let mut endpoint_dto = EndpointDto::new();
        let mut ec_dto = ErrorCodeDto {
            val: 0,
            category: 0,
        };
        unsafe {
            REMOTE_ENDPOINT_CALLBACK.expect("REMOTE_ENDPOINT_CALLBACK missing")(
                self.handle,
                &mut endpoint_dto,
                &mut ec_dto,
            );
        }
        if ec_dto.val == 0 {
            Ok((&endpoint_dto).into())
        } else {
            Err((&ec_dto).into())
        }
    }

    fn post(&self, f: Box<dyn FnOnce() + Send>) {
        unsafe {
            POST_CALLBACK.expect("POST_CALLBACK missing")(
                self.handle,
                Box::into_raw(Box::new(VoidFnCallbackHandle::new(f))),
            );
        }
    }

    fn dispatch(&self, f: Box<dyn FnOnce() + Send>) {
        unsafe {
            DISPATCH_CALLBACK.expect("DISPATCH_CALLBACK missing")(
                self.handle,
                Box::into_raw(Box::new(VoidFnCallbackHandle::new(f))),
            );
        }
    }

    fn close(&self) -> Result<(), ErrorCode> {
        let mut ec_dto = ErrorCodeDto {
            val: 0,
            category: 0,
        };
        unsafe {
            CLOSE_SOCKET_CALLBACK.expect("CLOSE_SOCKET_CALLBACK missing")(self.handle, &mut ec_dto);
        }

        if ec_dto.val == 0 {
            Ok(())
        } else {
            Err((&ec_dto).into())
        }
    }

    fn local_endpoint(&self) -> SocketAddr {
        unsafe {
            let mut dto = EndpointDto::new();
            LOCAL_ENDPOINT_CALLBACK.expect("LOCAL_ENDPOINT_CALLBACK missing")(
                self.handle,
                &mut dto,
            );
            SocketAddr::from(&dto)
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn is_open(&self) -> bool {
        unsafe { SOCKET_IS_OPEN_CALLBACK.expect("SOCKET_IS_OPEN_CALLBACK missing")(self.handle) }
    }

    fn close_acceptor(&self) {
        unsafe {
            SOCKET_CLOSE_ACCEPTOR_CALLBACK.expect("SOCKET_CLOSE_ACCEPTOR_CALLBACK missing")(
                self.handle,
            )
        }
    }

    fn is_acceptor_open(&self) -> bool {
        unsafe { IS_ACCEPTOR_OPEN.expect("IS_ACCEPTOR_OPEN missing")(self.handle) }
    }

    fn async_accept(
        &self,
        client_socket: &Arc<dyn TcpSocketFacade>,
        callback: Box<dyn FnOnce(SocketAddr, ErrorCode) + Send>,
    ) {
        let callback_handle = Box::into_raw(Box::new(AsyncAcceptCallbackHandle(Some(callback))));
        unsafe {
            ASYNC_ACCEPT_CALLBACK.expect("ASYNC_ACCEPT_CALLBACK missing")(
                self.handle,
                client_socket
                    .as_any()
                    .downcast_ref::<FfiTcpSocketFacade>()
                    .unwrap()
                    .handle,
                callback_handle,
            )
        };
    }

    fn open(&self, endpoint: &SocketAddr) -> ErrorCode {
        let mut error = ErrorCodeDto {
            val: 0,
            category: 0,
        };
        let endpoint_dto = EndpointDto::from(endpoint);
        unsafe {
            TCP_SOCKET_OPEN.expect("TCP_SOCKET_OPEN missing")(
                self.handle,
                &endpoint_dto,
                &mut error,
            );
        }
        (&error).into()
    }

    fn listening_port(&self) -> u16 {
        unsafe {
            TCP_SOCKET_LISTENING_PORT.expect("TCP_SOCKET_LISTENING_PORT missing")(self.handle)
        }
    }
}

unsafe impl Send for FfiTcpSocketFacade {}
unsafe impl Sync for FfiTcpSocketFacade {}

static mut BUFFER_DESTROY_CALLBACK: Option<VoidPointerCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_buffer_destroy(f: VoidPointerCallback) {
    BUFFER_DESTROY_CALLBACK = Some(f);
}

type BufferSizeCallback = unsafe extern "C" fn(*mut c_void) -> usize;
static mut BUFFER_SIZE_CALLBACK: Option<BufferSizeCallback> = None;

type BufferGetSliceCallback = unsafe extern "C" fn(*mut c_void) -> *mut u8;
static mut BUFFER_GET_SLICE: Option<BufferGetSliceCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_buffer_size(f: BufferSizeCallback) {
    BUFFER_SIZE_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_buffer_get_slice(f: BufferGetSliceCallback) {
    BUFFER_GET_SLICE = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_read_callback_execute(
    callback: *mut AsyncReadCallbackHandle,
    ec: *const ErrorCodeDto,
    size: usize,
) {
    let error_code = ErrorCode::from(&*ec);
    (*callback).0.take().unwrap()(error_code, size);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_read_callback_destroy(callback: *mut AsyncReadCallbackHandle) {
    drop(Box::from_raw(callback))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_write_callback_execute(
    callback: *mut AsyncWriteCallbackHandle,
    ec: *const ErrorCodeDto,
    size: usize,
) {
    let error_code = ErrorCode::from(&*ec);
    if let Some(cb) = (*callback).0.take() {
        cb(error_code, size);
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_write_callback_destroy(callback: *mut AsyncWriteCallbackHandle) {
    drop(Box::from_raw(callback))
}

struct FfiBufferWrapper {
    handle: *mut c_void,
}

impl FfiBufferWrapper {
    fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

unsafe impl Send for FfiBufferWrapper {}
unsafe impl Sync for FfiBufferWrapper {}

impl Drop for FfiBufferWrapper {
    fn drop(&mut self) {
        unsafe { BUFFER_DESTROY_CALLBACK.expect("BUFFER_DESTROY_CALLBACK missing")(self.handle) }
    }
}

impl BufferWrapper for FfiBufferWrapper {
    fn len(&self) -> usize {
        unsafe { BUFFER_SIZE_CALLBACK.expect("BUFFER_SIZE_CALLBACK missing")(self.handle) }
    }

    fn handle(&self) -> *mut c_void {
        self.handle
    }

    fn get_slice_mut(&self) -> &mut [u8] {
        unsafe {
            let ptr = BUFFER_GET_SLICE.expect("BUFFER_GET_SLICE missing")(self.handle);
            let len = self.len();
            std::slice::from_raw_parts_mut(ptr, len)
        }
    }
}

pub struct SocketFfiObserver {
    handle: *mut c_void,
}

impl SocketFfiObserver {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

unsafe impl Send for SocketFfiObserver {}
unsafe impl Sync for SocketFfiObserver {}

impl SocketObserver for SocketFfiObserver {
    fn socket_connected(&self, socket: Arc<Socket>) {
        unsafe {
            SOCKET_CONNECTED_CALLBACK.expect("SOCKET_CONNECTED_CALLBACK missing")(
                self.handle,
                SocketHandle::new(socket),
            )
        }
    }

    fn socket_accepted(&self, socket: Arc<Socket>) {
        unsafe {
            SOCKET_ACCEPTED_CALLBACK.expect("SOCKET_ACCEPTED_CALLBACK missing")(
                self.handle,
                SocketHandle::new(socket),
            )
        }
    }
}

impl Drop for SocketFfiObserver {
    fn drop(&mut self) {
        unsafe {
            DELETE_TCP_SOCKET_CALLBACK.expect("DELETE_TCP_SOCKET_CALLBACK missing")(self.handle)
        }
    }
}

#[derive(Clone)]
#[repr(C)]
pub struct EndpointDto {
    pub bytes: [u8; 16],
    pub port: u16,
    pub v6: bool,
}

impl EndpointDto {
    pub fn new() -> EndpointDto {
        EndpointDto {
            bytes: [0; 16],
            port: 0,
            v6: false,
        }
    }
}

impl Default for EndpointDto {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&EndpointDto> for SocketAddrV6 {
    fn from(dto: &EndpointDto) -> Self {
        if dto.v6 {
            SocketAddrV6::new(Ipv6Addr::from(dto.bytes), dto.port, 0, 0)
        } else {
            panic!("not a v6 ip address")
        }
    }
}

impl From<SocketAddrV6> for EndpointDto {
    fn from(value: SocketAddrV6) -> Self {
        Self {
            bytes: value.ip().octets(),
            port: value.port(),
            v6: true,
        }
    }
}

impl From<&EndpointDto> for SocketAddr {
    fn from(dto: &EndpointDto) -> Self {
        let ip = if dto.v6 {
            IpAddr::V6(Ipv6Addr::from(dto.bytes))
        } else {
            let mut bytes = [0; 4];
            bytes.copy_from_slice(&dto.bytes[..4]);
            IpAddr::V4(Ipv4Addr::from(bytes))
        };

        SocketAddr::new(ip, dto.port)
    }
}

impl From<&SocketAddr> for EndpointDto {
    fn from(addr: &SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(a) => {
                let mut dto = EndpointDto {
                    bytes: [0; 16],
                    port: a.port(),
                    v6: false,
                };
                dto.bytes[..4].copy_from_slice(&a.ip().octets());
                dto
            }
            SocketAddr::V6(a) => EndpointDto {
                bytes: a.ip().octets(),
                port: a.port(),
                v6: true,
            },
        }
    }
}

impl From<SocketAddr> for EndpointDto {
    fn from(addr: SocketAddr) -> Self {
        EndpointDto::from(&addr)
    }
}
