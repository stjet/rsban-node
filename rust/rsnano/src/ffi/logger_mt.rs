use std::ffi::c_void;

use crate::Logger;

pub(crate) type TryLogCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> bool;
pub(crate) static mut TRY_LOG_CALLBACK: Option<TryLogCallback> = None;

pub(crate) struct LoggerMT {
    handle: *mut c_void,
}

impl LoggerMT {
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
}
#[no_mangle]
pub unsafe extern "C" fn rsn_callback_try_log(f: TryLogCallback) {
    TRY_LOG_CALLBACK = Some(f);
}
