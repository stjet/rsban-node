use crate::{ErrorCode, Socket, SocketImpl, TcpSocketFacade};
use std::{
    ffi::c_void,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    ops::Deref,
    sync::{atomic::Ordering, Arc, Mutex},
};

use super::StatHandle;

pub struct SocketHandle(Arc<Mutex<SocketImpl>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_create(
    tcp_facade: *mut c_void,
    stats_handle: *mut StatHandle,
) -> *mut SocketHandle {
    let tcp_facade = Arc::new(FfiTcpSocketFacade::new(tcp_facade));
    Box::into_raw(Box::new(SocketHandle(Arc::new(Mutex::new(
        SocketImpl::new(tcp_facade, (*stats_handle).deref().clone()),
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
    (*handle).0.lock().unwrap().remote = Some(SocketAddr::from(&*endpoint));
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
    match &(*handle).0.lock().unwrap().remote {
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
        .lock()
        .unwrap()
        .last_completion_time_or_init
        .load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_last_completion(handle: *mut SocketHandle) {
    (*handle).0.lock().unwrap().set_last_completion();
}

pub struct AsyncConnectCallbackHandle(Box<dyn Fn(ErrorCode)>);
type AsyncConnectCallback =
    unsafe extern "C" fn(*mut c_void, *const EndpointDto, *mut AsyncConnectCallbackHandle);

static mut ASYNC_CONNECT_CALLBACK: Option<AsyncConnectCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_async_connect(f: AsyncConnectCallback) {
    ASYNC_CONNECT_CALLBACK = Some(f);
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
                None => panic!(" ASYNC_CONNECT_CALLBACK missing"),
            }
        }
    }
}
