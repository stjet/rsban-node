use std::{
    ffi::{c_void, CStr, CString},
    os::raw::c_char,
    sync::Arc,
};

use num::FromPrimitive;

use crate::{
    websocket::{Listener, NullListener},
    BootstrapAttempt,
};

use super::{FfiListener, LoggerMT, StringDto, StringHandle};

pub struct BootstrapAttemptHandle(BootstrapAttempt);

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_create(
    logger: *mut c_void,
    websocket_server: *mut c_void,
    id: *const c_char,
    mode: u8,
) -> *mut BootstrapAttemptHandle {
    let logger = Arc::new(LoggerMT::new(logger));
    let id_str = CStr::from_ptr(id).to_str().unwrap();
    let mode = FromPrimitive::from_u8(mode).unwrap();
    let websocket_server: Arc<dyn Listener> = if websocket_server.is_null() {
        Arc::new(NullListener::new())
    } else {
        Arc::new(FfiListener::new(websocket_server))
    };
    Box::into_raw(Box::new(BootstrapAttemptHandle(
        BootstrapAttempt::new(logger, websocket_server, id_str, mode).unwrap(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_destroy(handle: *mut BootstrapAttemptHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_id(
    handle: *const BootstrapAttemptHandle,
    result: *mut StringDto,
) {
    let id = CString::new((*handle).0.id.as_str()).unwrap();
    let string_handle = Box::new(StringHandle(id));
    let result = &mut (*result);
    result.value = string_handle.0.as_ptr();
    result.handle = Box::into_raw(string_handle);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attemt_should_log(
    handle: *const BootstrapAttemptHandle,
) -> bool {
    (*handle).0.should_log()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attemt_bootstrap_mode(
    handle: *const BootstrapAttemptHandle,
    len: *mut usize,
) -> *const c_char {
    let mode_text = (*handle).0.mode_text();
    *len = mode_text.len();
    mode_text.as_ptr() as *const c_char
}
