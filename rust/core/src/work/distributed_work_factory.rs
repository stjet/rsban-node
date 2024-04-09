use std::ffi::c_void;

use crate::BlockEnum;

pub struct DistributedWorkFactory {
    /// Pointer to the C++ implementation
    factory_pointer: *mut c_void,
}

impl DistributedWorkFactory {
    pub fn new(factory_pointer: *mut c_void) -> Self {
        Self { factory_pointer }
    }

    pub fn make_blocking(&self, block: &BlockEnum, difficulty: u64) -> Option<u64> {
        unsafe {
            MAKE_BLOCKING.expect("MAKE_BLOCKING missing")(self.factory_pointer, block, difficulty)
        }
    }
}

pub static mut MAKE_BLOCKING: Option<fn(*mut c_void, &BlockEnum, u64) -> Option<u64>> = None;

unsafe impl Send for DistributedWorkFactory {}
unsafe impl Sync for DistributedWorkFactory {}
