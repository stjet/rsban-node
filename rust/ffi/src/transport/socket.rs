use num::FromPrimitive;

use rsnano_node::{
    stats::SocketStats,
    transport::{
        alive_sockets, CompositeSocketObserver, Socket, SocketBuilder, SocketExtensions,
        SocketObserver, WriteCallback,
    },
    utils::ErrorCode,
};
use std::{
    ffi::c_void,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6},
    ops::Deref,
    sync::Arc,
    time::Duration,
};
use tracing::debug;

use crate::{
    utils::{AsyncRuntimeHandle, ThreadPoolHandle},
    ErrorCodeDto, StatHandle, VoidPointerCallback,
};

pub struct SocketHandle(pub Arc<Socket>);

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
pub extern "C" fn rsn_sockets_alive() -> usize {
    alive_sockets()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_create(
    endpoint_type: u8,
    stats_handle: *mut StatHandle,
    thread_pool: &ThreadPoolHandle,
    default_timeout_s: u64,
    silent_connection_tolerance_time_s: u64,
    idle_timeout_s: u64,
    callback_handler: *mut c_void,
    max_write_queue_len: usize,
    async_rt: &AsyncRuntimeHandle,
) -> *mut SocketHandle {
    let endpoint_type = FromPrimitive::from_u8(endpoint_type).unwrap();
    let thread_pool = thread_pool.0.clone();
    let stats = (*stats_handle).deref().clone();

    let socket_stats = Arc::new(SocketStats::new(stats));
    let ffi_observer = Arc::new(SocketFfiObserver::new(callback_handler));

    let runtime = Arc::downgrade(&async_rt.0);
    let socket = SocketBuilder::new(endpoint_type, thread_pool, runtime)
        .default_timeout(Duration::from_secs(default_timeout_s))
        .silent_connection_tolerance_time(Duration::from_secs(silent_connection_tolerance_time_s))
        .idle_timeout(Duration::from_secs(idle_timeout_s))
        .observer(Arc::new(CompositeSocketObserver::new(vec![
            socket_stats,
            ffi_observer,
        ])))
        .max_write_queue_len(max_write_queue_len)
        .finish();
    debug!(socket_id = socket.socket_id, "Socket created from FFI");

    SocketHandle::new(socket)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_destroy(handle: *mut SocketHandle) {
    drop(Box::from_raw(handle))
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
    let buffer = if buffer.is_null() {
        &[]
    } else {
        std::slice::from_raw_parts(buffer, buffer_len)
    };
    (*handle).async_write(
        &Arc::new(buffer.to_vec()),
        cb,
        FromPrimitive::from_u8(traffic_type).unwrap(),
    );
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
pub unsafe extern "C" fn rsn_socket_checkup(handle: *mut SocketHandle) {
    (*handle).ongoing_checkup();
}

pub struct AsyncWriteCallbackHandle(Option<Box<dyn FnOnce(ErrorCode, usize)>>);

type SocketConnectedCallback = unsafe extern "C" fn(*mut c_void, *mut SocketHandle);
static mut SOCKET_CONNECTED_CALLBACK: Option<SocketConnectedCallback> = None;
static mut DELETE_TCP_SOCKET_CALLBACK: Option<VoidPointerCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_connected(f: SocketConnectedCallback) {
    SOCKET_CONNECTED_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_delete_tcp_socket_callback(f: VoidPointerCallback) {
    DELETE_TCP_SOCKET_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_default_timeout_value(handle: *mut SocketHandle) -> u64 {
    (*handle).default_timeout_value()
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

impl From<&SocketAddrV6> for EndpointDto {
    fn from(value: &SocketAddrV6) -> Self {
        Self {
            bytes: value.ip().octets(),
            port: value.port(),
            v6: true,
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
