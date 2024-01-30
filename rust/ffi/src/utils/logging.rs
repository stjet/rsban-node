use super::AlwaysLogCallback;
use crate::VoidPointerCallback;
use rsnano_core::utils::{LogLevel, LogType, Logger};
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
        self.log(LogLevel::Info, LogType::All, message);
    }

    fn log(&self, level: LogLevel, tag: LogType, message: &str) {
        unsafe {
            LOG_V2_CALLBACK.expect("LOG_V2_CALLBACK missing")(
                self.handle,
                level as u8,
                tag as u8,
                message.as_ptr(),
                message.len(),
            );
        }
    }

    fn debug(&self, tag: LogType, message: &str) {
        self.log(LogLevel::Debug, tag, message);
    }

    fn info(&self, tag: LogType, message: &str) {
        self.log(LogLevel::Info, tag, message);
    }

    fn warn(&self, tag: LogType, message: &str) {
        self.log(LogLevel::Warn, tag, message);
    }

    fn error(&self, tag: LogType, message: &str) {
        self.log(LogLevel::Error, tag, message);
    }

    fn critical(&self, tag: LogType, message: &str) {
        self.log(LogLevel::Critical, tag, message);
    }
}

unsafe impl Send for FfiLoggerV2 {}
unsafe impl Sync for FfiLoggerV2 {}

pub static mut DESTROY_LOGGER_V2: Option<VoidPointerCallback> = None;

pub type LogV2Callback = unsafe extern "C" fn(*mut c_void, level: u8, tag: u8, *const u8, usize);

pub static mut LOG_V2_CALLBACK: Option<LogV2Callback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_logger_v2_destroy(f: VoidPointerCallback) {
    DESTROY_LOGGER_V2 = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_logger_v2_log(f: LogV2Callback) {
    LOG_V2_CALLBACK = Some(f);
}
