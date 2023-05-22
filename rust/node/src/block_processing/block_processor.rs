use rsnano_core::BlockEnum;
use std::{
    ffi::c_void,
    sync::{Arc, Condvar, Mutex, RwLock},
};

pub static mut BLOCKPROCESSOR_ADD_CALLBACK: Option<fn(*mut c_void, Arc<RwLock<BlockEnum>>)> = None;

pub struct BlockProcessor {
    handle: *mut c_void,
    pub mutex: Mutex<()>,
    pub condition: Condvar,
}

impl BlockProcessor {
    pub fn new(handle: *mut c_void) -> Self {
        Self {
            handle,
            mutex: Mutex::new(()),
            condition: Condvar::new(),
        }
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
