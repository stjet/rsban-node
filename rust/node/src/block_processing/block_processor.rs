use rsnano_core::BlockEnum;
use std::{
    collections::VecDeque,
    ffi::c_void,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, SystemTime},
};

use crate::config::NodeConfig;

pub static mut BLOCKPROCESSOR_ADD_CALLBACK: Option<fn(*mut c_void, Arc<BlockEnum>)> = None;
pub static mut BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK: Option<fn(*mut c_void, Arc<BlockEnum>)> =
    None;
pub static mut BLOCKPROCESSOR_HALF_FULL_CALLBACK: Option<
    unsafe extern "C" fn(*mut c_void) -> bool,
> = None;

pub struct BlockProcessor {
    handle: *mut c_void,
    pub mutex: Mutex<BlockProcessorImpl>,
    pub condition: Condvar,
}

impl BlockProcessor {
    pub fn new(handle: *mut c_void, config: NodeConfig) -> Self {
        Self {
            handle,
            mutex: Mutex::new(BlockProcessorImpl {
                blocks: VecDeque::new(),
                forced: VecDeque::new(),
                next_log: SystemTime::now(),
                config,
            }),
            condition: Condvar::new(),
        }
    }

    pub fn process_active(&self, block: Arc<BlockEnum>) {
        unsafe {
            BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK
                .expect("BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK missing")(
                self.handle, block
            )
        }
    }

    pub fn add(&self, block: Arc<BlockEnum>) {
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

pub struct BlockProcessorImpl {
    pub blocks: VecDeque<Arc<BlockEnum>>,
    pub forced: VecDeque<Arc<BlockEnum>>,
    pub next_log: SystemTime,
    config: NodeConfig,
}

impl BlockProcessorImpl {
    pub fn should_log(&mut self) -> bool {
        let now = SystemTime::now();
        if self.next_log < now {
            let delay = if self.config.logging.timing_logging_value {
                Duration::from_secs(2)
            } else {
                Duration::from_secs(15)
            };
            self.next_log = now + delay;
            true
        } else {
            false
        }
    }
}
