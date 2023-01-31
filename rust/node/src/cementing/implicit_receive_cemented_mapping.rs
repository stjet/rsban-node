use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
};

use rsnano_core::BlockHash;

use super::ConfHeightDetails;

pub(crate) struct ImplictReceiveCementedMapping {
    //todo: Remove Mutex
    mapping: HashMap<BlockHash, Weak<Mutex<ConfHeightDetails>>>,
    // The atomic variable here just tracks the size for use in collect_container_info.
    // This is so that no mutexes are needed during the algorithm itself, which would otherwise be needed
    // for the sake of a rarely used RPC call for debugging purposes. As such the sizes are not being acted
    // upon in any way (does not synchronize with any other data).
    // This allows the load and stores to use relaxed atomic memory ordering.
    mapping_size: AtomicUsize,
}

impl ImplictReceiveCementedMapping {
    pub fn new() -> Self {
        Self {
            mapping: HashMap::new(),
            mapping_size: AtomicUsize::new(0),
        }
    }

    pub fn add(&mut self, hash: BlockHash, details: &Arc<Mutex<ConfHeightDetails>>) {
        self.mapping.insert(hash, Arc::downgrade(&details));
        self.mapping_size
            .store(self.mapping.len(), Ordering::Relaxed);
    }

    pub fn get(&self, hash: &BlockHash) -> Option<&Weak<Mutex<ConfHeightDetails>>> {
        self.mapping.get(hash)
    }

    pub fn clear(&mut self) {
        self.mapping.clear();
        self.mapping_size.store(0, Ordering::Relaxed);
    }

    pub fn size_atomic(&self) -> usize {
        self.mapping_size.load(Ordering::Relaxed)
    }
}
