use std::ffi::c_void;

pub trait IoContext: Send + Sync {
    fn post(&self, f: Box<dyn FnOnce()>);
    fn raw_handle(&self) -> *mut c_void;
}
