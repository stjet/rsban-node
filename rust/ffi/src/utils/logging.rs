use crate::VoidPointerCallback;
use num_traits::FromPrimitive;
use rsnano_core::utils::{LogLevel, LogType, Logger};
use std::{
    ffi::{c_char, c_void},
    sync::Arc,
};
use tracing::{enabled, event, Level};
use tracing_subscriber::EnvFilter;

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

#[derive(FromPrimitive)]
enum CppLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
    Off,
}

impl From<CppLogLevel> for tracing::Level {
    fn from(value: CppLogLevel) -> Self {
        match value {
            CppLogLevel::Trace => Level::TRACE,
            CppLogLevel::Debug => Level::DEBUG,
            CppLogLevel::Info => Level::INFO,
            CppLogLevel::Warn => Level::WARN,
            CppLogLevel::Error => Level::ERROR,
            CppLogLevel::Critical => Level::ERROR,
            CppLogLevel::Off => Level::TRACE,
        }
    }
}

#[no_mangle]
pub extern "C" fn rsn_log_init() {
    let dirs = std::env::var(EnvFilter::DEFAULT_ENV).unwrap_or(String::from(
        "rsnano_ffi=debug,rsnano_node=debug,rsnano_messages=debug,rsnano_ledger=debug,rsnano_store_lmdb=debug,rsnano_core=debug",
    ));
    let filter = EnvFilter::builder().parse_lossy(dirs);

    tracing_subscriber::fmt::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_log(log_level: u8, message: *const c_char, len: usize) {
    let message = std::mem::transmute::<*const c_char, *const u8>(message);
    let data = std::slice::from_raw_parts(message, len);
    let message = std::str::from_utf8(data).unwrap();

    let cpp_level: CppLogLevel = FromPrimitive::from_u8(log_level).unwrap();
    let level = Level::from(cpp_level);

    //TODO log tag as well
    if level == Level::TRACE {
        event!(Level::TRACE, message);
    } else if level == Level::DEBUG {
        event!(Level::DEBUG, message);
    } else if level == Level::INFO {
        event!(Level::INFO, message);
    } else if level == Level::WARN {
        event!(Level::WARN, message);
    } else if level == Level::ERROR {
        event!(Level::ERROR, message);
    }
}

#[no_mangle]
pub extern "C" fn rsn_log_min_level() -> u8 {
    let cpp_level = if enabled!(Level::TRACE) {
        CppLogLevel::Trace
    } else if enabled!(Level::DEBUG) {
        CppLogLevel::Debug
    } else if enabled!(Level::INFO) {
        CppLogLevel::Info
    } else if enabled!(Level::WARN) {
        CppLogLevel::Warn
    } else if enabled!(Level::ERROR) {
        CppLogLevel::Error
    } else {
        CppLogLevel::Off
    };

    cpp_level as u8
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
