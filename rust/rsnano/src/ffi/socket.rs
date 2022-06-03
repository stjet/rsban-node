use crate::Socket;
use std::ffi::c_void;

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

type SocketConnectCallback = unsafe extern "C" fn(*mut c_void, *const ErrorCodeDto);

#[no_mangle]
pub unsafe extern "C" fn rsn_socket_async_connect(
    _handle: *mut SocketHandle,
    callback: SocketConnectCallback,
    context: *mut c_void,
    error_code: *const ErrorCodeDto,
    _endpoint: *const EndpointDto,
) {
    callback(context, error_code);
}
