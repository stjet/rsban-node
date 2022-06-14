use num::FromPrimitive;

use crate::{BufferWrapper, ErrorCode, Socket, SocketImpl, TcpSocketFacade};
use std::{
    ffi::c_void,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    ops::Deref,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use super::{
    thread_pool::{FfiThreadPool, VoidFnCallbackHandle},
    LoggerMT, StatHandle,
};

pub struct SocketHandle(Arc<SocketImpl>);

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_create(
    endpoint_type: u8,
    tcp_facade: *mut c_void,
    stats_handle: *mut StatHandle,
    thread_pool: *mut c_void,
    default_timeout_s: u64,
    silent_connection_tolerance_time_s: u64,
    network_timeout_logging: bool,
    logger: *mut c_void,
) -> *mut SocketHandle {
    let endpoint_type = FromPrimitive::from_u8(endpoint_type).unwrap();
    let tcp_facade = Arc::new(FfiTcpSocketFacade::new(tcp_facade));
    let thread_pool = Arc::new(FfiThreadPool::new(thread_pool));
    Box::into_raw(Box::new(SocketHandle(Arc::new(SocketImpl::new(
        endpoint_type,
        tcp_facade,
        (*stats_handle).deref().clone(),
        thread_pool,
        Duration::from_secs(default_timeout_s),
        Duration::from_secs(silent_connection_tolerance_time_s),
        network_timeout_logging,
        Arc::new(LoggerMT::new(logger)),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_destroy(handle: *mut SocketHandle) {
    drop(Box::from_raw(handle))
}

#[repr(C)]
pub struct ErrorCodeDto {
    pub val: i32,
    pub category: u8,
}

impl From<&ErrorCode> for ErrorCodeDto {
    fn from(ec: &ErrorCode) -> Self {
        Self {
            val: ec.val,
            category: ec.category,
        }
    }
}

impl From<&ErrorCodeDto> for ErrorCode {
    fn from(dto: &ErrorCodeDto) -> Self {
        Self {
            val: dto.val,
            category: dto.category,
        }
    }
}

#[repr(C)]
pub struct EndpointDto {
    pub bytes: [u8; 16],
    pub port: u16,
    pub v6: bool,
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

type SocketConnectCallback = unsafe extern "C" fn(*mut c_void, *const ErrorCodeDto);
type SocketDestroyContext = unsafe extern "C" fn(*mut c_void);

struct ConnectCallbackWrapper {
    callback: SocketConnectCallback,
    destory_context: SocketDestroyContext,
    context: *mut c_void,
}

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
    (*handle).0.async_connect((&*endpoint).into(), cb);
}

struct ReadCallbackWrapper {
    callback: SocketReadCallback,
    destory_context: SocketDestroyContext,
    context: *mut c_void,
}

impl ReadCallbackWrapper {
    fn new(
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
    fn execute(&self, ec: ErrorCode, size: usize) {
        let ec_dto = ErrorCodeDto::from(&ec);
        unsafe { (self.callback)(self.context, &ec_dto, size) };
    }
}

impl Drop for ReadCallbackWrapper {
    fn drop(&mut self) {
        unsafe { (self.destory_context)(self.context) };
    }
}

type SocketReadCallback = unsafe extern "C" fn(*mut c_void, *const ErrorCodeDto, usize);

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
    (*handle).0.async_read(buffer_wrapper, size, cb);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_remote_endpoint(
    handle: *mut SocketHandle,
    endpoint: *const EndpointDto,
) {
    let mut lk = (*handle).0.remote.lock().unwrap();
    *lk = Some(SocketAddr::from(&*endpoint));
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
    match (*handle).0.remote.lock().unwrap().as_ref() {
        Some(ep) => {
            set_enpoint_dto(ep, &mut *result);
        }
        None => {
            (*result).port = 0;
            (*result).v6 = false;
            (*result).bytes = [0; 16];
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_get_last_completion_time(handle: *mut SocketHandle) -> u64 {
    (*handle)
        .0
        .last_completion_time_or_init
        .load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_last_completion(handle: *mut SocketHandle) {
    (*handle).0.set_last_completion();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_get_last_receive_time(handle: *mut SocketHandle) -> u64 {
    (*handle).0.last_receive_time_or_init.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_last_receive_time(handle: *mut SocketHandle) {
    (*handle).0.set_last_receive_time();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_get_silent_connnection_tolerance_time_s(
    handle: *mut SocketHandle,
) -> u64 {
    (*handle)
        .0
        .silent_connection_tolerance_time
        .load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_silent_connection_tolerance_time(
    handle: *mut SocketHandle,
    time_s: u64,
) {
    (*handle)
        .0
        .silent_connection_tolerance_time
        .store(time_s, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_timeout(handle: *mut SocketHandle, timeout_s: u64) {
    (*handle).0.set_timeout(Duration::from_secs(timeout_s));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_get_timeout_s(handle: *mut SocketHandle) -> u64 {
    (*handle).0.timeout_seconds.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_close(handle: *mut SocketHandle) {
    (*handle).0.close()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_close_internal(handle: *mut SocketHandle) {
    (*handle).0.close_internal();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_is_closed(handle: *mut SocketHandle) -> bool {
    (*handle).0.is_closed()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_has_timed_out(handle: *mut SocketHandle) -> bool {
    (*handle).0.timed_out.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_checkup(handle: *mut SocketHandle) {
    (*handle).0.checkup();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_get_queue_size(handle: *mut SocketHandle) -> usize {
    (*handle).0.queue_size.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_queue_size_inc(handle: *mut SocketHandle) {
    (*handle).0.queue_size.fetch_add(1, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_queue_size_dec(handle: *mut SocketHandle) {
    (*handle).0.queue_size.fetch_sub(1, Ordering::SeqCst);
}

pub struct AsyncConnectCallbackHandle(Box<dyn Fn(ErrorCode)>);
type AsyncConnectCallback =
    unsafe extern "C" fn(*mut c_void, *const EndpointDto, *mut AsyncConnectCallbackHandle);

static mut ASYNC_CONNECT_CALLBACK: Option<AsyncConnectCallback> = None;

type RemoteEndpointCallback =
    unsafe extern "C" fn(*mut c_void, *mut EndpointDto, *mut ErrorCodeDto);

static mut REMOTE_ENDPOINT_CALLBACK: Option<RemoteEndpointCallback> = None;

type DispatchCallback = unsafe extern "C" fn(*mut c_void, *mut VoidFnCallbackHandle);

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

pub struct AsyncReadCallbackHandle(Box<dyn Fn(ErrorCode, usize)>);

type AsyncReadCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, usize, *mut AsyncReadCallbackHandle);

static mut ASYNC_READ_CALLBACK: Option<AsyncReadCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_async_read(f: AsyncReadCallback) {
    ASYNC_READ_CALLBACK = Some(f);
}

type DestroyCallback = unsafe extern "C" fn(*mut c_void);

static mut TCP_FACADE_DESTROY_CALLBACK: Option<DestroyCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_destroy(f: DestroyCallback) {
    TCP_FACADE_DESTROY_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_connect_callback_execute(
    callback: *mut AsyncConnectCallbackHandle,
    ec: *const ErrorCodeDto,
) {
    let error_code = ErrorCode::from(&*ec);
    (*callback).0(error_code);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_connect_callback_destroy(
    callback: *mut AsyncConnectCallbackHandle,
) {
    drop(Box::from_raw(callback))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_default_timeout(handle: *mut SocketHandle) {
    (*handle).0.set_default_timeout();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_default_timeout_value(
    handle: *mut SocketHandle,
    timeout_s: u64,
) {
    (*handle)
        .0
        .default_timeout
        .store(timeout_s, Ordering::SeqCst);
}

struct FfiTcpSocketFacade {
    handle: *mut c_void,
}

impl FfiTcpSocketFacade {
    fn new(handle: *mut c_void) -> Self {
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
    fn async_connect(&self, endpoint: SocketAddr, callback: Box<dyn Fn(ErrorCode)>) {
        let endpoint_dto = EndpointDto::from(&endpoint);
        let callback_handle = Box::new(AsyncConnectCallbackHandle(callback));
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
        callback: Box<dyn Fn(ErrorCode, usize)>,
    ) {
        let callback_handle = Box::into_raw(Box::new(AsyncReadCallbackHandle(callback)));
        unsafe {
            ASYNC_READ_CALLBACK.expect("ASYNC_READ_CALLBACK missing")(
                self.handle,
                buffer.handle(),
                len,
                callback_handle,
            );
        }
    }

    fn remote_endpoint(&self) -> Result<SocketAddr, ErrorCode> {
        let mut endpoint_dto = EndpointDto {
            bytes: [0; 16],
            port: 0,
            v6: false,
        };
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

    fn post(&self, f: Box<dyn Fn()>) {
        unsafe {
            POST_CALLBACK.expect("POST_CALLBACK missing")(
                self.handle,
                Box::into_raw(Box::new(VoidFnCallbackHandle::new(f))),
            );
        }
    }

    fn dispatch(&self, f: Box<dyn Fn()>) {
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
}

static mut BUFFER_DESTROY_CALLBACK: Option<DestroyCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_buffer_destroy(f: DestroyCallback) {
    BUFFER_DESTROY_CALLBACK = Some(f);
}

type BufferSizeCallback = unsafe extern "C" fn(*mut c_void) -> usize;

static mut BUFFER_SIZE_CALLBACK: Option<BufferSizeCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_buffer_size(f: BufferSizeCallback) {
    BUFFER_SIZE_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_read_callback_execute(
    callback: *mut AsyncReadCallbackHandle,
    ec: *const ErrorCodeDto,
    size: usize,
) {
    let error_code = ErrorCode::from(&*ec);
    (*callback).0(error_code, size);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_read_callback_destroy(callback: *mut AsyncReadCallbackHandle) {
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
}
