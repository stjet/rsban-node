use rsnano_core::UncheckedInfo;
use std::ffi::c_void;

pub static mut BLOCKPROCESSOR_ADD_CALLBACK: Option<fn(*mut c_void, &UncheckedInfo)> = None;

pub struct BlockProcessor {
    handle: *mut c_void,
}

impl BlockProcessor {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }

    pub fn add(&self, info: &UncheckedInfo) {
        unsafe {
            match BLOCKPROCESSOR_ADD_CALLBACK {
                Some(f) => f(self.handle, info),
                None => panic!("BLOCKPROCESSOR_ADD_CALLBACK missing"),
            }
        }
    }
}
