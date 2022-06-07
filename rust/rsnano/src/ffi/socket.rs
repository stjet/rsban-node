use num::FromPrimitive;

use crate::{ErrorCode, Socket, SocketImpl, TcpSocketFacade};
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

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_async_connect(
    handle: *mut SocketHandle,
    endpoint: *const EndpointDto,
    callback: SocketConnectCallback,
    context: *mut c_void,
) {
    let cb = Box::new(move |ec| {
        let ec_dto = ErrorCodeDto::from(&ec);
        callback(context, &ec_dto);
    });

    (*handle).0.async_connect((&*endpoint).into(), cb);
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
    (*handle).0.closed.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_has_timed_out(handle: *mut SocketHandle) -> bool {
    (*handle).0.timed_out.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_checkup(handle: *mut SocketHandle) {
    (*handle).0.checkup();
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

type CloseSocketCallback = unsafe extern "C" fn(*mut c_void, *mut ErrorCodeDto);

static mut CLOSE_SOCKET_CALLBACK: Option<CloseSocketCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_async_connect(f: AsyncConnectCallback) {
    ASYNC_CONNECT_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_remote_endpoint(f: RemoteEndpointCallback) {
    REMOTE_ENDPOINT_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_dispatch(f: DispatchCallback) {
    DISPATCH_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_tcp_socket_close(f: CloseSocketCallback) {
    CLOSE_SOCKET_CALLBACK = Some(f);
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

struct FfiTcpSocketFacade {
    handle: *mut c_void,
}

impl FfiTcpSocketFacade {
    fn new(handle: *mut c_void) -> Self {
        Self { handle }
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
