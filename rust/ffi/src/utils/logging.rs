use super::AlwaysLogCallback;
use crate::VoidPointerCallback;
use rsnano_core::utils::Logger;
use std::{ffi::c_void, sync::Arc};

pub struct LoggerHandleV2(*mut c_void);

impl LoggerHandleV2 {
    pub fn into_logger(&self) -> Arc<dyn Logger> {
        Arc::new(FfiLoggerV2::new(self.0))
    }
}

/// logger is a pointer to a shared_ptr<nlogger>
#[no_mangle]
pub unsafe extern "C" fn rsn_logger_create_v2(logger: *mut c_void) -> *mut LoggerHandleV2 {
    Box::into_raw(Box::new(LoggerHandleV2(logger)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_logger_destroy_v2(handle: *mut LoggerHandleV2) {
    drop(Box::from_raw(handle));
}

pub struct FfiLoggerV2 {
    handle: *mut c_void,
}

impl FfiLoggerV2 {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

impl Drop for FfiLoggerV2 {
    fn drop(&mut self) {
        unsafe { DESTROY_LOGGER_V2.expect("DESTROY_LOGGER_V2 missing")(self.handle) }
    }
}

impl Logger for FfiLoggerV2 {
    fn try_log(&self, message: &str) -> bool {
        self.always_log(message);
        true
    }

    fn always_log(&self, message: &str) {
        unsafe {
            LOG_INFO_CALLBACK.expect("LOG_INFO_CALLBACK missing")(
                self.handle,
                message.as_ptr(),
                message.len(),
            );
        }
    }
}

unsafe impl Send for FfiLoggerV2 {}
unsafe impl Sync for FfiLoggerV2 {}

pub static mut DESTROY_LOGGER_V2: Option<VoidPointerCallback> = None;

pub type LogInfoCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize);
pub static mut LOG_INFO_CALLBACK: Option<LogInfoCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_logger_v2_destroy(f: VoidPointerCallback) {
    DESTROY_LOGGER_V2 = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_logger_v2_info(f: AlwaysLogCallback) {
    LOG_INFO_CALLBACK = Some(f);
}
