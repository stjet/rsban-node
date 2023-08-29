use std::ffi::c_void;

pub struct AsyncRuntime {
    pub cpp: *mut c_void,
    pub tokio: tokio::runtime::Runtime,
}
