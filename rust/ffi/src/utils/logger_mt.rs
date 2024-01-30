use crate::VoidPointerCallback;
use rsnano_core::utils::{LogLevel, LogType, Logger};
use std::ffi::c_void;

pub type TryLogCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> bool;
pub static mut TRY_LOG_CALLBACK: Option<TryLogCallback> = None;

pub type AlwaysLogCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize);
pub static mut ALWAYS_LOG_CALLBACK: Option<AlwaysLogCallback> = None;

pub(crate) struct LoggerMT {
    /// handle is a `nano::logger_mt *`
    handle: Box<LoggerHandle>,
}

impl LoggerMT {
    /// handle is a `nano::logger_mt *`
    pub(crate) fn new(handle: Box<LoggerHandle>) -> Self {
        Self { handle }
    }
}

unsafe impl Sync for LoggerMT {}
unsafe impl Send for LoggerMT {}

impl Logger for LoggerMT {
    fn try_log(&self, message: &str) -> bool {
        unsafe {
            match TRY_LOG_CALLBACK {
                Some(log) => log(self.handle.0, message.as_ptr(), message.len()),
                None => panic!("TRY_LOG_CALLBACK not defined"),
            }
        }
    }

    fn always_log(&self, message: &str) {
        unsafe {
            match ALWAYS_LOG_CALLBACK {
                Some(log) => log(self.handle.0, message.as_ptr(), message.len()),
                None => panic!("ALWAYS_LOG_CALLBACK not defined"),
            }
        }
    }

    fn log(&self, _level: LogLevel, _tag: LogType, message: &str) {
        self.always_log(message);
    }

    fn debug(&self, _tag: LogType, message: &str) {
        self.always_log(message);
    }

    fn info(&self, _tag: LogType, message: &str) {
        self.always_log(message);
    }

    fn warn(&self, _tag: LogType, message: &str) {
        self.always_log(message);
    }

    fn error(&self, _tag: LogType, message: &str) {
        self.always_log(message);
    }

    fn critical(&self, _tag: LogType, message: &str) {
        self.always_log(message);
    }
}

impl Drop for LoggerMT {
    fn drop(&mut self) {
        unsafe { DESTROY_LOGGER_HANDLE.expect("DESTROY_LOGGER_HANDLE missing")(self.handle.0) }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_try_log(f: TryLogCallback) {
    TRY_LOG_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_always_log(f: AlwaysLogCallback) {
    ALWAYS_LOG_CALLBACK = Some(f);
}

/// points to a shared_ptr<logger_mt>
#[derive(Copy, Clone)]
pub struct LoggerHandle(pub *mut c_void);

/// logger is a pointer to a shared_ptr<logger_mt>
#[no_mangle]
pub unsafe extern "C" fn rsn_logger_create(logger: *mut c_void) -> *mut LoggerHandle {
    Box::into_raw(Box::new(LoggerHandle(logger)))
}

pub static mut DESTROY_LOGGER_HANDLE: Option<VoidPointerCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_logger_destroy(f: VoidPointerCallback) {
    DESTROY_LOGGER_HANDLE = Some(f);
}
