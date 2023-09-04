use std::{ffi::c_void, sync::Arc};

pub struct AsyncRuntime {
    pub cpp: *mut c_void,
    pub tokio: Arc<tokio::runtime::Runtime>,
}
