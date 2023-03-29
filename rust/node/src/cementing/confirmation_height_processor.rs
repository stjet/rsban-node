use std::{
    collections::HashSet,
    mem::size_of,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex, MutexGuard,
    },
    thread::JoinHandle,
    time::Duration,
};

use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent, Latch, Logger},
    BlockEnum, BlockHash,
};
use rsnano_ledger::{Ledger, WriteDatabaseQueue};

use crate::{config::Logging, stats::Stats};

use super::{
    block_cache::BlockCache, AutomaticMode, AutomaticModeContainerInfo, BlockQueue, BoundedMode,
    ConfirmationHeightMode, NotifyObserversCallback, UnboundedMode,
};

pub struct ConfirmationHeightProcessor {
    channel: Arc<Mutex<ProcessorLoopChannel>>,
    condition: Arc<Condvar>,
    /** The maximum amount of blocks to write at once. This is dynamically modified by the bounded processor based on previous write performance **/
    batch_write_size: Arc<AtomicU64>,
    stopped: Arc<AtomicBool>,
    // No mutex needed for the observers as these should be set up during initialization of the node
    cemented_observer: Arc<Mutex<Option<Box<dyn Fn(&Arc<BlockEnum>) + Send>>>>,
    already_cemented_observer: Arc<Mutex<Option<Box<dyn Fn(BlockHash) + Send>>>>,
    thread: Option<JoinHandle<()>>,
    block_cache: Arc<BlockCache>,

    automatic_container_info: AutomaticModeContainerInfo,
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
        let channel = Arc::new(Mutex::new(ProcessorLoopChannel {
            paused: false,
            awaiting_processing: BlockQueue::new(),
            pending_writes: HashSet::new(),
            current_block: None,
        }));

        let bounded_processor = BoundedMode::new(
            write_database_queue.clone(),
            cemented_callback(&cemented_observer),
            block_already_cemented_callback(&already_cemented_observer),
            batch_write_size.clone(),
            logger.clone(),
            logging.clone(),
            ledger.clone(),
            stopped.clone(),
            batch_separate_pending_min_time,
            awaiting_processing_size_callback(&channel),
        );

        let block_cache = Arc::new(BlockCache::new(ledger.clone()));

        let unbounded_processor = UnboundedMode::new(
            ledger.clone(),
            logger,
            logging,
            stats,
            batch_separate_pending_min_time,
            batch_write_size.clone(),
            write_database_queue.clone(),
            cemented_callback(&cemented_observer),
            block_already_cemented_callback(&already_cemented_observer),
            awaiting_processing_size_callback(&channel),
            block_cache.clone(),
            stopped.clone(),
        );

        let condition = Arc::new(Condvar::new());
        let processor = AutomaticMode {
            bounded_processor,
            unbounded_processor,
            mode,
            ledger,
        };

        let automatic_container_info = processor.container_info();

        let join_handle = {
            let stopped = stopped.clone();
            let condition = condition.clone();
            let channel = channel.clone();

            std::thread::Builder::new()
                .name("Conf height".to_owned())
                .spawn(move || {
                    let mut processor_loop = ConfirmationHeightProcessorLoop {
                        stopped,
                        condition,
                        processor,
                        channel: &channel,
                    };
                    // Do not start running the processing thread until other threads have finished their operations
                    latch.wait();
                    processor_loop.run();
                })
                .unwrap()
        };

        Self {
            channel,
            condition,
            batch_write_size,
            stopped,
            cemented_observer,
            already_cemented_observer,
            thread: Some(join_handle),
            block_cache,
            automatic_container_info,
        }
    }

    // Pausing only affects processing new blocks, not the current one being processed. Currently only used in tests
    pub fn pause(&self) {
        let mut guard = self.channel.lock().unwrap();
        guard.paused = true;
    }

    pub fn unpause(&self) {
        let mut guard = self.channel.lock().unwrap();
        guard.paused = false;
        drop(guard);
        self.condition.notify_one();
    }

    pub fn set_batch_write_size(&self, size: usize) {
        self.batch_write_size.store(size as u64, Ordering::SeqCst);
    }

    pub fn add(&self, block: Arc<BlockEnum>) {
        {
            let mut lk = self.channel.lock().unwrap();
            lk.awaiting_processing.push_back(block);
        }
        self.condition.notify_one();
    }

    pub fn current(&self) -> BlockHash {
        let lk = self.channel.lock().unwrap();
        match &lk.current_block {
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
        let lk = self.channel.lock().unwrap();
        lk.pending_writes.contains(block_hash) || lk.awaiting_processing.contains(block_hash)
    }

    pub fn awaiting_processing_len(&self) -> usize {
        let lk = self.channel.lock().unwrap();
        lk.awaiting_processing.len()
    }

    pub fn stop(&mut self) {
        {
            let _guard = self.channel.lock().unwrap(); //todo why is this needed?
            self.stopped.store(true, Ordering::SeqCst);
        }
        self.condition.notify_one();
        if let Some(handle) = self.thread.take() {
            handle.join().unwrap();
        }
    }

    pub fn collect_container_info(&self, name: String) -> ContainerInfoComponent {
        let mut children = vec![ContainerInfoComponent::Leaf(ContainerInfo {
            name: "awaiting_processing".to_owned(),
            count: self.awaiting_processing_len(),
            sizeof_element: size_of::<usize>(),
        })];

        children.append(&mut self.automatic_container_info.collect());

        ContainerInfoComponent::Composite(name, children)
    }
}

impl Drop for ConfirmationHeightProcessor {
    fn drop(&mut self) {
        self.stop();
    }
}

fn awaiting_processing_size_callback(
    channel: &Arc<Mutex<ProcessorLoopChannel>>,
) -> Box<dyn Fn() -> u64 + Send> {
    let channel_clone = Arc::clone(channel);

    let awaiting_processing_size_callback = Box::new(move || {
        let lk = channel_clone.lock().unwrap();
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

/// Used for inter thread communication between ConfirmationHeightProcessor and ConfirmationHeightProcessorLoop
struct ProcessorLoopChannel {
    pub paused: bool,
    pub awaiting_processing: BlockQueue,
    /// Hashes which have been added and processed, but have not been cemented
    pub pending_writes: HashSet<BlockHash>,
    /// This is the last block popped off the awaiting_processing queue
    pub current_block: Option<Arc<BlockEnum>>,
}

impl ProcessorLoopChannel {
    fn clear_processed_blocks(&mut self) {
        self.current_block = None;
        self.pending_writes.clear();
    }
}

struct ConfirmationHeightProcessorLoop<'a> {
    stopped: Arc<AtomicBool>,
    condition: Arc<Condvar>,
    processor: AutomaticMode,
    channel: &'a Mutex<ProcessorLoopChannel>,
}

impl<'a> ConfirmationHeightProcessorLoop<'a> {
    pub fn run(&mut self) {
        let mut channel = self.channel.lock().unwrap();
        while !self.stopped.load(Ordering::SeqCst) {
            if channel.paused {
                channel = self.pause(channel);
            } else if let Some(block) = channel.awaiting_processing.pop_front() {
                channel = self.process_block(channel, block);
            } else {
                channel = self.flush_remaining_writes(channel);
            }
        }
    }

    fn pause(
        &self,
        mut channel: MutexGuard<'a, ProcessorLoopChannel>,
    ) -> MutexGuard<'a, ProcessorLoopChannel> {
        // Pausing is only utilised in some tests to help prevent it processing added blocks until required.
        channel.current_block = None;
        self.condition.wait(channel).unwrap()
    }

    fn process_block(
        &mut self,
        mut channel: MutexGuard<'a, ProcessorLoopChannel>,
        block: Arc<BlockEnum>,
    ) -> MutexGuard<'a, ProcessorLoopChannel> {
        if self.processor.pending_writes_empty() {
            channel.pending_writes.clear();
        }

        channel.pending_writes.insert(block.hash());
        channel.current_block = Some(block.clone());

        drop(channel);
        self.processor.process(block);
        self.channel.lock().unwrap()
    }

    /// If there are blocks pending cementing, then make sure we flush out the remaining writes
    fn flush_remaining_writes(
        &mut self,
        mut channel: MutexGuard<'a, ProcessorLoopChannel>,
    ) -> MutexGuard<'a, ProcessorLoopChannel> {
        if !self.processor.pending_writes_empty() {
            drop(channel);
            self.processor.write_pending_blocks();
            channel = self.channel.lock().unwrap();
        }

        channel.clear_processed_blocks();
        self.processor.clear_process_vars();

        // A block could have been confirmed during the re-locking
        if channel.awaiting_processing.is_empty() {
            channel = self.condition.wait(channel).unwrap();
        }
        channel
    }
}
