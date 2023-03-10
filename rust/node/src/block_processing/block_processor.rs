use rsnano_core::BlockEnum;
use std::{
    ffi::c_void,
    sync::{Arc, RwLock},
};

pub static mut BLOCKPROCESSOR_ADD_CALLBACK: Option<fn(*mut c_void, Arc<RwLock<BlockEnum>>)> = None;

pub struct BlockProcessor {
    handle: *mut c_void,
}

impl BlockProcessor {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }

    pub fn add(&self, block: Arc<RwLock<BlockEnum>>) {
        unsafe {
            match BLOCKPROCESSOR_ADD_CALLBACK {
                Some(f) => f(self.handle, block),
                None => panic!("BLOCKPROCESSOR_ADD_CALLBACK missing"),
            }
        }
    }
}
