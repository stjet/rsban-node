mod stream;
use std::ffi::c_void;

pub use stream::FfiStream;

mod toml;
pub use toml::FfiToml;

mod thread_pool;
pub use thread_pool::{FfiThreadPool, VoidFnCallbackHandle};
mod io_context;
pub use io_context::{DispatchCallback, FfiIoContext, IoContextHandle};
mod logger_mt;
pub use logger_mt::*;

use crate::VoidPointerCallback;

pub struct ContextWrapper {
    context: *mut c_void,
    drop_context: VoidPointerCallback,
}

impl ContextWrapper {
    pub fn new(context: *mut c_void, drop_context: VoidPointerCallback) -> Self {
        Self {
            context,
            drop_context,
        }
    }

    pub fn get_context(&self) -> *mut c_void {
        self.context
    }
}

unsafe impl Send for ContextWrapper {}
unsafe impl Sync for ContextWrapper {}

impl Drop for ContextWrapper {
    fn drop(&mut self) {
        unsafe {
            (self.drop_context)(self.context);
        }
    }
}
