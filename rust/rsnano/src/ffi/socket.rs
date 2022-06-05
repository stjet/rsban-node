use crate::{ErrorCode, Socket};
use std::{
    ffi::c_void,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    ops::Deref,
    sync::atomic::Ordering,
};

use super::StatHandle;

pub struct SocketHandle(Socket);

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_create(stats_handle: *mut StatHandle) -> *mut SocketHandle {
    Box::into_raw(Box::new(SocketHandle(Socket::new(
        (*stats_handle).deref().clone(),
    ))))
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

type SocketConnectCallback = unsafe extern "C" fn(*mut c_void, *const ErrorCodeDto);

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_async_connect(
    handle: *mut SocketHandle,
    callback: SocketConnectCallback,
    context: *mut c_void,
    error_code: *const ErrorCodeDto,
    endpoint: *const EndpointDto,
) {
    let cb = Box::new(move |ec| {
        let ec_dto = ErrorCodeDto::from(&ec);
        callback(context, &ec_dto);
    });

    (*handle)
        .0
        .async_connect((&*endpoint).into(), (&*error_code).into(), cb)
        .unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_remote_endpoint(
    handle: *mut SocketHandle,
    endpoint: *const EndpointDto,
) {
    (*handle).0.remote = Some(SocketAddr::from(&*endpoint));
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
    match &(*handle).0.remote {
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
