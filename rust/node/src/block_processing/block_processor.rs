use rsnano_core::BlockEnum;
use std::{
    ffi::c_void,
    sync::{Arc, Condvar, Mutex, RwLock},
};

pub static mut BLOCKPROCESSOR_ADD_CALLBACK: Option<fn(*mut c_void, Arc<RwLock<BlockEnum>>)> = None;
pub static mut BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK: Option<fn(*mut c_void, Arc<RwLock<BlockEnum>>)> = None;
pub static mut BLOCKPROCESSOR_HALF_FULL_CALLBACK: Option<
    unsafe extern "C" fn(*mut c_void) -> bool,
> = None;

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

    pub fn process_active(&self, block: Arc<RwLock<BlockEnum>>) {
        unsafe {
            BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK.expect("BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK missing")(
                self.handle,
                block,
            )
        }
    }

    pub fn add(&self, block: Arc<RwLock<BlockEnum>>) {
        unsafe {
            BLOCKPROCESSOR_ADD_CALLBACK.expect("BLOCKPROCESSOR_ADD_CALLBACK missing")(
                self.handle,
                block,
            )
        }
    }

    pub fn half_full(&self) -> bool {
        unsafe {
            BLOCKPROCESSOR_HALF_FULL_CALLBACK.expect("BLOCKPROCESSOR_ADD_CALLBACK missing")(
                self.handle,
            )
        }
    }
}

unsafe impl Send for BlockProcessor {}
unsafe impl Sync for BlockProcessor {}
