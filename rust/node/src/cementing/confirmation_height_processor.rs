use std::{
    collections::{HashSet, VecDeque},
    mem::size_of,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc, Condvar, Mutex, RwLock,
    },
    time::Duration,
};

use rsnano_core::{utils::Logger, BlockEnum, BlockHash};
use rsnano_ledger::{Ledger, WriteDatabaseQueue};

use crate::config::{ConfirmationHeightMode, Logging};

use super::{ConfirmationHeightBounded, NotifyObserversCallback};

pub struct ConfirmationHeightProcessor {
    pub guarded_data: Arc<Mutex<GuardedData>>,
    pub condition: Arc<Condvar>,
    write_database_queue: Arc<WriteDatabaseQueue>,
    /** The maximum amount of blocks to write at once. This is dynamically modified by the bounded processor based on previous write performance **/
    pub batch_write_size: Arc<AtomicU64>,
    pub bounded_processor: ConfirmationHeightBounded,
    pub stopped: Arc<AtomicBool>,
    // No mutex needed for the observers as these should be set up during initialization of the node
    cemented_observer: Arc<Mutex<Option<Box<dyn Fn(&Arc<RwLock<BlockEnum>>)>>>>, //todo remove Arc<Mutex<>>
    already_cemented_observer: Arc<Mutex<Option<Box<dyn Fn(BlockHash)>>>>, //todo remove Arc<Mutex<>>
}

impl ConfirmationHeightProcessor {
    pub fn new(
        write_database_queue: Arc<WriteDatabaseQueue>,
        logger: Arc<dyn Logger>,
        logging: Logging,
        ledger: Arc<Ledger>,
        batch_separate_pending_min_time: Duration,
    ) -> Self {
        let cemented_observer: Arc<Mutex<Option<Box<dyn Fn(&Arc<RwLock<BlockEnum>>)>>>> =
            Arc::new(Mutex::new(None));
        let cemented_observer_clone = Arc::clone(&cemented_observer);
        let cemented_callback: NotifyObserversCallback = Box::new(move |blocks| {
            let lock = cemented_observer_clone.lock().unwrap();
            if let Some(f) = lock.deref() {
                for block in blocks {
                    (f)(block);
                }
            }
        });

        let already_cemented_observer: Arc<Mutex<Option<Box<dyn Fn(BlockHash)>>>> =
            Arc::new(Mutex::new(None));
        let already_cemented_observer_clone = Arc::clone(&already_cemented_observer);
        let already_cemented_callback = Box::new(move |block_hash| {
            let lock = already_cemented_observer_clone.lock().unwrap();
            if let Some(f) = lock.deref() {
                (f)(block_hash);
            }
        });

        let awaiting_processing_size_callback = Box::new(|| todo!());

        let batch_write_size = Arc::new(AtomicU64::new(16384));
        let stopped = Arc::new(AtomicBool::new(false));
        Self {
            guarded_data: Arc::new(Mutex::new(GuardedData {
                paused: false,
                awaiting_processing: AwaitingProcessingQueue::new(),
                original_hashes_pending: HashSet::new(),
                original_block: None,
            })),
            condition: Arc::new(Condvar::new()),
            write_database_queue: write_database_queue.clone(),
            batch_write_size: batch_write_size.clone(),
            stopped: stopped.clone(),
            cemented_observer,
            already_cemented_observer,
            bounded_processor: ConfirmationHeightBounded::new(
                write_database_queue,
                cemented_callback,
                already_cemented_callback,
                batch_write_size,
                logger,
                logging,
                ledger,
                stopped,
                batch_separate_pending_min_time,
                awaiting_processing_size_callback,
            ),
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

    pub fn add(&self, block: Arc<RwLock<BlockEnum>>) {
        {
            let mut lk = self.guarded_data.lock().unwrap();
            lk.awaiting_processing.push_back(block);
        }
        self.condition.notify_one();
    }

    pub fn awaiting_processing_entry_size() -> usize {
        AwaitingProcessingQueue::entry_size()
    }

    pub fn set_next_hash(&self) {
        let mut lk = self.guarded_data.lock().unwrap();
        debug_assert!(!lk.awaiting_processing.is_empty());
        let block = lk.awaiting_processing.front().unwrap().clone();
        lk.original_hashes_pending
            .insert(block.read().unwrap().hash());
        lk.original_block = Some(block);
        lk.awaiting_processing.pop_front();
    }

    pub fn current(&self) -> BlockHash {
        let lk = self.guarded_data.lock().unwrap();
        match &lk.original_block {
            Some(block) => block.read().unwrap().hash(),
            None => BlockHash::zero(),
        }
    }

    pub fn run(&self, _mode: ConfirmationHeightMode) {
        //todo
    }

    pub fn set_cemented_observer(&mut self, callback: Box<dyn Fn(&Arc<RwLock<BlockEnum>>)>) {
        *self.cemented_observer.lock().unwrap() = Some(callback);
    }

    pub fn set_already_cemented_observer(&mut self, callback: Box<dyn Fn(BlockHash)>) {
        *self.already_cemented_observer.lock().unwrap() = Some(callback);
    }

    pub fn notify_cemented(&self, blocks: &[Arc<RwLock<BlockEnum>>]) {
        let lock = self.cemented_observer.lock().unwrap();
        if let Some(observer) = lock.deref() {
            for block in blocks {
                (observer)(block);
            }
        }
    }

    pub fn notify_already_cemented(&self, block_hash: &BlockHash) {
        let lock = self.already_cemented_observer.lock().unwrap();
        if let Some(observer) = lock.deref() {
            (observer)(*block_hash);
        }
    }

    pub fn clear_cemented_observer(&mut self) {
        *self.cemented_observer.lock().unwrap() = None;
    }

    pub fn is_processing_added_block(&self, block_hash: &BlockHash) -> bool {
        let lk = self.guarded_data.lock().unwrap();
        lk.original_hashes_pending.contains(block_hash)
            || lk.awaiting_processing.contains(block_hash)
    }

    pub fn awaiting_processing_len(&self) -> usize {
        let lk = self.guarded_data.lock().unwrap();
        lk.awaiting_processing.len()
    }
}

pub struct GuardedData {
    pub paused: bool,
    pub awaiting_processing: AwaitingProcessingQueue,
    // Hashes which have been added and processed, but have not been cemented
    pub original_hashes_pending: HashSet<BlockHash>,
    /** This is the last block popped off the confirmation height pending collection */
    pub original_block: Option<Arc<RwLock<BlockEnum>>>,
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

    pub fn push_back(&mut self, block: Arc<RwLock<BlockEnum>>) {
        let hash = block.read().unwrap().hash();
        if self.hashes.contains(&hash) {
            return;
        }

        self.blocks.push_back(block);
        self.hashes.insert(hash);
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
