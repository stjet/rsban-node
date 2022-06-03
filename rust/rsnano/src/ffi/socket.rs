use crate::Socket;
use std::{ffi::c_void, net::{SocketAddr, IpAddr, Ipv6Addr, Ipv4Addr}};

pub struct SocketHandle(Socket);

#[no_mangle]
pub extern "C" fn rsn_socket_create() -> *mut SocketHandle {
    Box::into_raw(Box::new(SocketHandle(Socket::new())))
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

#[repr(C)]
pub struct EndpointDto {
    pub bytes: [u8; 16],
    pub port: u16,
    pub v6: bool,
}

impl From<&EndpointDto> for SocketAddr{
    fn from(dto: &EndpointDto) -> Self {
        let ip = if dto.v6{
            IpAddr::V6(Ipv6Addr::from(dto.bytes))
        } else{
            let mut bytes = [0;4];
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
    (*handle).0.async_connect((&*endpoint).into());
    callback(context, error_code);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_set_remote_endpoint(
    handle: *mut SocketHandle,
    endpoint: *const EndpointDto){
        (*handle).0.remote = Some(SocketAddr::from(&*endpoint));
    }

fn set_enpoint_dto(endpoint: &SocketAddr, result: &mut EndpointDto){
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
pub unsafe extern "C" fn rsn_socket_get_remote(handle: *mut SocketHandle, result: *mut EndpointDto) {
    match &(*handle).0.remote {
        Some(ep) => {
            set_enpoint_dto (ep, &mut *result);
        },
        None => {
            (*result).port = 0;
            (*result).v6 = false;
            (*result).bytes = [0;16];
        }
    }
}