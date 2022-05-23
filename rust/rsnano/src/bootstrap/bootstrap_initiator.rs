use std::ffi::c_void;

pub struct BootstrapInitiator {
    handle: *mut c_void,
}

impl BootstrapInitiator {
    pub(crate) fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}
