use std::{
    collections::{HashSet, VecDeque},
    mem::size_of,
    sync::{Arc, Condvar, Mutex, RwLock},
};

use rsnano_core::{BlockEnum, BlockHash};
use rsnano_ledger::WriteDatabaseQueue;

pub struct ConfirmationHeightProcessor {
    pub guarded_data: Arc<Mutex<GuardedData>>,
    pub condition: Arc<Condvar>,
    write_database_queue: Arc<WriteDatabaseQueue>,
}

impl ConfirmationHeightProcessor {
    pub fn new(write_database_queue: Arc<WriteDatabaseQueue>) -> Self {
        Self {
            guarded_data: Arc::new(Mutex::new(GuardedData {
                paused: false,
                awaiting_processing: AwaitingProcessingQueue::new(),
                original_hashes_pending: HashSet::new(),
            })),
            condition: Arc::new(Condvar::new()),
            write_database_queue,
        }
    }

    // Pausing only affects processing new blocks, not the current one being processed. Currently only used in tests
    pub fn pause(&self) {
        let mut guard = self.guarded_data.lock().unwrap();
        guard.paused = true;
    }

    pub fn unpause(&self) {
        let mut guard = self.guarded_data.lock().unwrap();
        guard.paused = false;
        drop(guard);
        self.condition.notify_one();
    }

    pub fn add(&self, block: Arc<RwLock<BlockEnum>>) -> anyhow::Result<()> {
        {
            let mut lk = self.guarded_data.lock().unwrap();
            lk.awaiting_processing.push_back(block)?;
        }
        self.condition.notify_one();
        Ok(())
    }

    pub fn awaiting_processing_entry_size() -> usize {
        AwaitingProcessingQueue::entry_size()
    }
}

pub struct GuardedData {
    pub paused: bool,
    pub awaiting_processing: AwaitingProcessingQueue,
    // Hashes which have been added and processed, but have not been cemented
    pub original_hashes_pending: HashSet<BlockHash>,
}

pub struct AwaitingProcessingQueue {
    blocks: VecDeque<Arc<RwLock<BlockEnum>>>,
    hashes: HashSet<BlockHash>,
}

impl AwaitingProcessingQueue {
    pub fn new() -> Self {
        Self {
            blocks: VecDeque::new(),
            hashes: HashSet::new(),
        }
    }

    pub fn entry_size() -> usize {
        size_of::<Arc<RwLock<BlockEnum>>>() + size_of::<BlockHash>()
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn contains(&self, hash: &BlockHash) -> bool {
        self.hashes.contains(hash)
    }

    pub fn push_back(&mut self, block: Arc<RwLock<BlockEnum>>) -> anyhow::Result<()> {
        let hash = block.read().unwrap().hash();
        if self.hashes.contains(&hash) {
            bail!("block was already in processing queue");
        }

        self.blocks.push_back(block);
        self.hashes.insert(hash);

        Ok(())
    }

    pub fn front(&self) -> Option<&Arc<RwLock<BlockEnum>>> {
        self.blocks.front()
    }

    pub fn pop_front(&mut self) -> Option<Arc<RwLock<BlockEnum>>> {
        let front = self.blocks.pop_front();
        if let Some(block) = &front {
            self.hashes.remove(&block.read().unwrap().hash());
        }
        front
    }
}
