use crate::{core::BlockHash, ffi::ledger::datastore::BLOCK_OR_PRUNED_EXISTS_CALLBACK};
use std::{
    ffi::c_void,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

pub struct Ledger {
    handle: *mut c_void,
    pruning: AtomicBool,
    bootstrap_weight_max_blocks: AtomicU64,
}

impl Ledger {
    pub fn new(handle: *mut c_void) -> Self {
        Self {
            handle,
            pruning: AtomicBool::new(false),
            bootstrap_weight_max_blocks: AtomicU64::new(1),
        }
    }

    pub fn pruning_enabled(&self) -> bool {
        self.pruning.load(Ordering::SeqCst)
    }

    pub fn enable_pruning(&self) {
        self.pruning.store(true, Ordering::SeqCst);
    }

    pub fn bootstrap_weight_max_blocks(&self) -> u64 {
        self.bootstrap_weight_max_blocks.load(Ordering::SeqCst)
    }

    pub fn set_bootstrap_weight_max_blocks(&self, max: u64) {
        self.bootstrap_weight_max_blocks
            .store(max, Ordering::SeqCst)
    }

    pub fn block_or_pruned_exists(&self, block: &BlockHash) -> bool {
        unsafe {
            match BLOCK_OR_PRUNED_EXISTS_CALLBACK {
                Some(f) => f(self.handle, block.as_bytes().as_ptr()),
                None => panic!("BLOCK_OR_PRUNED_EXISTS_CALLBACK missing"),
            }
        }
    }
}
