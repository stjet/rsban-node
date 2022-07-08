use std::ffi::c_void;

use crate::Logger;

pub type TryLogCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> bool;
pub static mut TRY_LOG_CALLBACK: Option<TryLogCallback> = None;

pub type AlwaysLogCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize);
pub static mut ALWAYS_LOG_CALLBACK: Option<AlwaysLogCallback> = None;

pub(crate) struct LoggerMT {
    /// handle is a `nano::logger_mt *`
    handle: *mut c_void,
}

impl LoggerMT {
    /// handle is a `nano::logger_mt *`
    pub(crate) fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

unsafe impl Sync for LoggerMT {}
unsafe impl Send for LoggerMT {}

impl Logger for LoggerMT {
    fn try_log(&self, message: &str) -> bool {
        unsafe {
            match TRY_LOG_CALLBACK {
                Some(log) => log(self.handle, message.as_ptr(), message.len()),
                None => panic!("TRY_LOG_CALLBACK not defined"),
            }
        }
    }

    fn always_log(&self, message: &str) {
        unsafe {
            match ALWAYS_LOG_CALLBACK {
                Some(log) => log(self.handle, message.as_ptr(), message.len()),
                None => panic!("ALWAYS_LOG_CALLBACK not defined"),
            }
        }
    }

    fn handle(&self) -> *mut c_void {
        self.handle
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
