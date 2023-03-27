use std::{
    collections::HashSet,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

use rsnano_core::{
    utils::{Latch, Logger},
    Account, BlockEnum, BlockHash,
};
use rsnano_ledger::{Ledger, WriteDatabaseQueue};

use crate::{
    config::{ConfirmationHeightMode, Logging},
    stats::Stats,
};

use super::{
    block_cache::BlockCache, BlockQueue, ConfirmationHeightBounded, ConfirmationHeightUnbounded,
    ConfirmedInfo, NotifyObserversCallback, WriteDetails,
};

/** When the uncemented count (block count - cemented count) is less than this use the unbounded processor */
const UNBOUNDED_CUTOFF: u64 = 16384;

pub struct ConfirmationHeightProcessor {
    guarded_data: Arc<Mutex<GuardedData>>,
    condition: Arc<Condvar>,
    /** The maximum amount of blocks to write at once. This is dynamically modified by the bounded processor based on previous write performance **/
    batch_write_size: Arc<AtomicU64>,
    stopped: Arc<AtomicBool>,
    // No mutex needed for the observers as these should be set up during initialization of the node
    cemented_observer: Arc<Mutex<Option<Box<dyn Fn(&Arc<BlockEnum>) + Send>>>>,
    already_cemented_observer: Arc<Mutex<Option<Box<dyn Fn(BlockHash) + Send>>>>,
    thread: Option<JoinHandle<()>>,
    block_cache: Arc<BlockCache>,
    pub unbounded_pending_writes: Arc<AtomicUsize>,
    pub bounded_accounts_confirmed: Arc<AtomicUsize>,
    pub bounded_pending_writes: Arc<AtomicUsize>,
    pub unbounded_confirmed_iterated_pairs_size: Arc<AtomicUsize>,
    pub unbounded_implicit_receive_cemented_mapping_size: Arc<AtomicUsize>,
}

impl ConfirmationHeightProcessor {
    pub fn new(
        write_database_queue: Arc<WriteDatabaseQueue>,
        logger: Arc<dyn Logger>,
        logging: Logging,
        ledger: Arc<Ledger>,
        batch_separate_pending_min_time: Duration,
        stats: Arc<Stats>,
        latch: Box<dyn Latch>,
        mode: ConfirmationHeightMode,
    ) -> Self {
        let cemented_observer: Arc<Mutex<Option<Box<dyn Fn(&Arc<BlockEnum>) + Send>>>> =
            Arc::new(Mutex::new(None));
        let already_cemented_observer: Arc<Mutex<Option<Box<dyn Fn(BlockHash) + Send>>>> =
            Arc::new(Mutex::new(None));
        let batch_write_size = Arc::new(AtomicU64::new(16384));
        let stopped = Arc::new(AtomicBool::new(false));
        let guarded_data = Arc::new(Mutex::new(GuardedData {
            paused: false,
            awaiting_processing: BlockQueue::new(),
            original_hashes_pending: HashSet::new(),
            original_block: None,
        }));

        let bounded_processor = ConfirmationHeightBounded::new(
            write_database_queue.clone(),
            cemented_callback(&cemented_observer),
            block_already_cemented_callback(&already_cemented_observer),
            batch_write_size.clone(),
            logger.clone(),
            logging.clone(),
            ledger.clone(),
            stopped.clone(),
            batch_separate_pending_min_time,
            awaiting_processing_size_callback(&guarded_data),
        );

        let bounded_accounts_confirmed = bounded_processor.accounts_confirmed_info_size.clone();
        let bounded_pending_writes = bounded_processor.pending_writes_size.clone();

        let block_cache = Arc::new(BlockCache::new(ledger.clone()));

        let unbounded_processor = ConfirmationHeightUnbounded::new(
            ledger.clone(),
            logger,
            logging,
            stats,
            batch_separate_pending_min_time,
            batch_write_size.clone(),
            write_database_queue.clone(),
            cemented_callback(&cemented_observer),
            block_already_cemented_callback(&already_cemented_observer),
            awaiting_processing_size_callback(&guarded_data),
            block_cache.clone(),
            stopped.clone(),
        );

        let unbounded_pending_writes = Arc::clone(unbounded_processor.pending_writes_size());
        let unbounded_confirmed_iterated_pairs_size =
            Arc::clone(unbounded_processor.confirmed_iterated_pairs_size_atomic());
        let unbounded_implicit_receive_cemented_mapping_size =
            Arc::clone(unbounded_processor.implicit_receive_cemented_mapping_size());

        let condition = Arc::new(Condvar::new());
        let mut thread = ConfirmationHeightProcessorThread {
            guarded_data: guarded_data.clone(),
            stopped: stopped.clone(),
            write_database_queue,
            ledger,
            condition: condition.clone(),
            bounded_processor,
            unbounded_processor,
            mode,
        };

        let join_handle = std::thread::Builder::new()
            .name("Conf height".to_owned())
            .spawn(move || {
                // Do not start running the processing thread until other threads have finished their operations
                latch.wait();
                thread.run();
            })
            .unwrap();

        Self {
            guarded_data,
            condition,
            batch_write_size,
            stopped,
            cemented_observer,
            already_cemented_observer,
            thread: Some(join_handle),
            block_cache,
            unbounded_pending_writes,
            bounded_accounts_confirmed,
            bounded_pending_writes,
            unbounded_confirmed_iterated_pairs_size,
            unbounded_implicit_receive_cemented_mapping_size,
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

    pub fn set_batch_write_size(&self, size: usize) {
        self.batch_write_size.store(size as u64, Ordering::SeqCst);
    }

    pub fn add(&self, block: Arc<BlockEnum>) {
        {
            let mut lk = self.guarded_data.lock().unwrap();
            lk.awaiting_processing.push_back(block);
        }
        self.condition.notify_one();
    }

    pub fn current(&self) -> BlockHash {
        let lk = self.guarded_data.lock().unwrap();
        match &lk.original_block {
            Some(block) => block.hash(),
            None => BlockHash::zero(),
        }
    }

    pub fn set_cemented_observer(&mut self, callback: Box<dyn Fn(&Arc<BlockEnum>) + Send>) {
        *self.cemented_observer.lock().unwrap() = Some(callback);
    }

    pub fn set_already_cemented_observer(&mut self, callback: Box<dyn Fn(BlockHash) + Send>) {
        *self.already_cemented_observer.lock().unwrap() = Some(callback);
    }

    pub fn clear_cemented_observer(&mut self) {
        *self.cemented_observer.lock().unwrap() = None;
    }

    pub fn is_processing_block(&self, block_hash: &BlockHash) -> bool {
        self.is_processing_added_block(block_hash) || self.block_cache.contains(block_hash)
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

    pub fn unbounded_pending_writes_len(&self) -> usize {
        self.unbounded_pending_writes.load(Ordering::Relaxed)
    }

    pub fn stop(&mut self) {
        {
            let _guard = self.guarded_data.lock().unwrap(); //todo why is this needed?
            self.stopped.store(true, Ordering::SeqCst);
        }
        self.condition.notify_one();
        if let Some(handle) = self.thread.take() {
            handle.join().unwrap();
        }
    }

    pub fn unbounded_block_cache_size(&self) -> usize {
        self.block_cache.len()
    }

    pub fn bounded_write_details_size() -> usize {
        std::mem::size_of::<WriteDetails>()
    }

    pub fn bounded_confirmed_info_entry_size() -> usize {
        std::mem::size_of::<ConfirmedInfo>() + std::mem::size_of::<Account>()
    }

    pub fn awaiting_processing_entry_size() -> usize {
        BlockQueue::entry_size()
    }
}

impl Drop for ConfirmationHeightProcessor {
    fn drop(&mut self) {
        self.stop();
    }
}

fn awaiting_processing_size_callback(
    guarded_data: &Arc<Mutex<GuardedData>>,
) -> Box<dyn Fn() -> u64 + Send> {
    let guarded_data_clone = Arc::clone(guarded_data);

    let awaiting_processing_size_callback = Box::new(move || {
        let lk = guarded_data_clone.lock().unwrap();
        lk.awaiting_processing.len() as u64
    });
    awaiting_processing_size_callback
}

fn block_already_cemented_callback(
    already_cemented_observer: &Arc<Mutex<Option<Box<dyn Fn(BlockHash) + Send>>>>,
) -> Box<dyn Fn(BlockHash) + Send> {
    let already_cemented_observer_clone = Arc::clone(already_cemented_observer);
    let already_cemented_callback = Box::new(move |block_hash| {
        let lock = already_cemented_observer_clone.lock().unwrap();
        if let Some(f) = lock.deref() {
            (f)(block_hash);
        }
    });
    already_cemented_callback
}

fn cemented_callback(
    cemented_observer: &Arc<Mutex<Option<Box<dyn Fn(&Arc<BlockEnum>) + Send>>>>,
) -> Box<dyn Fn(&Vec<Arc<BlockEnum>>) + Send> {
    let cemented_observer_clone = Arc::clone(cemented_observer);
    let cemented_callback: NotifyObserversCallback = Box::new(move |blocks| {
        let lock = cemented_observer_clone.lock().unwrap();
        if let Some(f) = lock.deref() {
            for block in blocks {
                (f)(block);
            }
        }
    });
    cemented_callback
}

struct GuardedData {
    pub paused: bool,
    pub awaiting_processing: BlockQueue,
    // Hashes which have been added and processed, but have not been cemented
    pub original_hashes_pending: HashSet<BlockHash>,
    /** This is the last block popped off the confirmation height pending collection */
    pub original_block: Option<Arc<BlockEnum>>,
}

struct ConfirmationHeightProcessorThread {
    guarded_data: Arc<Mutex<GuardedData>>,
    stopped: Arc<AtomicBool>,
    write_database_queue: Arc<WriteDatabaseQueue>,
    ledger: Arc<Ledger>,
    condition: Arc<Condvar>,
    pub bounded_processor: ConfirmationHeightBounded,
    pub unbounded_processor: ConfirmationHeightUnbounded,
    mode: ConfirmationHeightMode,
}

impl ConfirmationHeightProcessorThread {
    pub fn run(&mut self) {
        let mut guard = self.guarded_data.lock().unwrap();
        while !self.stopped.load(Ordering::SeqCst) {
            if !guard.paused && !guard.awaiting_processing.is_empty() {
                if self.bounded_processor.pending_empty()
                    && self.unbounded_processor.pending_empty()
                {
                    guard.original_hashes_pending.clear();
                }

                self.set_next_hash(&mut guard);
                if let Some(original_block) = &guard.original_block {
                    let original_block = Arc::clone(original_block);

                    drop(guard);

                    // Don't want to mix up pending writes across different processors
                    if self.should_use_unbounded_processor() {
                        self.unbounded_processor.process(original_block);
                    } else {
                        self.bounded_processor.process(original_block.deref());
                    }

                    guard = self.guarded_data.lock().unwrap();
                }
            } else {
                if !guard.paused {
                    drop(guard);

                    // If there are blocks pending cementing, then make sure we flush out the remaining writes
                    if !self.bounded_processor.pending_empty() {
                        debug_assert!(self.unbounded_processor.pending_empty());

                        {
                            let mut scoped_write_guard = self
                                .write_database_queue
                                .wait(rsnano_ledger::Writer::ConfirmationHeight);
                            self.bounded_processor
                                .cement_blocks(&mut scoped_write_guard);
                        }
                        guard = self.guarded_data.lock().unwrap();
                        guard.original_block = None;
                        guard.original_hashes_pending.clear();
                        self.bounded_processor.clear_process_vars();
                        self.unbounded_processor.clear_process_vars();
                    } else if !self.unbounded_processor.pending_empty() {
                        debug_assert!(self.bounded_processor.pending_empty());
                        {
                            let _scoped_write_guard = self
                                .write_database_queue
                                .wait(rsnano_ledger::Writer::ConfirmationHeight);
                            //todo why is scoped_write_guard not being used in Rust version????
                            self.unbounded_processor.cement_pending_blocks();
                        }
                        guard = self.guarded_data.lock().unwrap();
                        guard.original_block = None;
                        guard.original_hashes_pending.clear();
                        self.bounded_processor.clear_process_vars();
                        self.unbounded_processor.clear_process_vars();
                    } else {
                        guard = self.guarded_data.lock().unwrap();
                        guard.original_block = None;
                        guard.original_hashes_pending.clear();
                        self.bounded_processor.clear_process_vars();
                        self.unbounded_processor.clear_process_vars();
                        // A block could have been confirmed during the re-locking
                        if guard.awaiting_processing.is_empty() {
                            guard = self.condition.wait(guard).unwrap();
                        }
                    }
                } else {
                    // Pausing is only utilised in some tests to help prevent it processing added blocks until required.
                    guard.original_block = None;
                    guard = self.condition.wait(guard).unwrap();
                }
            }
        }
    }

    fn should_use_unbounded_processor(&self) -> bool {
        let valid_unbounded = self.valid_unbounded();
        let force_unbounded = self.force_unbounded();

        let use_unbounded_processor = force_unbounded || valid_unbounded;
        use_unbounded_processor
    }

    fn force_unbounded(&self) -> bool {
        !self.unbounded_processor.pending_empty() || self.mode == ConfirmationHeightMode::Unbounded
    }

    fn valid_unbounded(&self) -> bool {
        self.mode == ConfirmationHeightMode::Automatic
            && self.are_blocks_within_automatic_unbounded_section()
            && self.bounded_processor.pending_empty()
    }

    fn are_blocks_within_automatic_unbounded_section(&self) -> bool {
        let block_count = self.ledger.cache.block_count.load(Ordering::SeqCst);
        let cemented_count = self.ledger.cache.cemented_count.load(Ordering::SeqCst);

        block_count < UNBOUNDED_CUTOFF || block_count - UNBOUNDED_CUTOFF < cemented_count
    }

    fn set_next_hash(&self, guard: &mut GuardedData) {
        debug_assert!(!guard.awaiting_processing.is_empty());
        let block = guard.awaiting_processing.front().unwrap().clone();
        guard.original_hashes_pending.insert(block.hash());
        guard.original_block = Some(block);
        guard.awaiting_processing.pop_front();
    }
}
