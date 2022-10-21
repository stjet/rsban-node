use crate::{
    core::{Account, BlockHash},
    ffi::ledger::datastore::BLOCK_OR_PRUNED_EXISTS_CALLBACK,
};
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use super::datastore::Store;

pub struct Ledger {
    handle: *mut c_void,
    store: Arc<dyn Store>,
    pruning: AtomicBool,
    bootstrap_weight_max_blocks: AtomicU64,
    pub check_bootstrap_weights: AtomicBool,
    pub bootstrap_weights: Mutex<HashMap<Account, u128>>,
}

impl Ledger {
    pub fn new(handle: *mut c_void, store: Arc<dyn Store>) -> Self {
        Self {
            handle,
            store,
            pruning: AtomicBool::new(false),
            bootstrap_weight_max_blocks: AtomicU64::new(1),
            check_bootstrap_weights: AtomicBool::new(true),
            bootstrap_weights: Mutex::new(HashMap::new()),
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
