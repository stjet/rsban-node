use super::UncheckedMap;
use crate::{
    config::{NodeConfig, NodeFlags},
    stats::{DetailType, Direction, StatType, Stats},
    transport::{ChannelEnum, FairQueue, Origin},
};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    work::{WorkThresholds, WORK_THRESHOLDS_STUB},
    BlockEnum, BlockType, Epoch, HashOrAccount, UncheckedInfo,
};
use rsnano_ledger::{BlockStatus, Ledger, Writer};
use rsnano_store_lmdb::LmdbWriteTransaction;
use std::{
    ffi::c_void,
    mem::size_of,
    sync::{atomic::AtomicBool, Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use tracing::{debug, error, info, trace};

pub static mut BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK: Option<fn(*mut c_void, Arc<BlockEnum>)> =
    None;

#[derive(FromPrimitive, Copy, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum BlockSource {
    Unknown = 0,
    Live,
    Bootstrap,
    BootstrapLegacy,
    Unchecked,
    Local,
    Forced,
}

impl From<BlockSource> for DetailType {
    fn from(value: BlockSource) -> Self {
        match value {
            BlockSource::Unknown => DetailType::Unknown,
            BlockSource::Live => DetailType::Live,
            BlockSource::Bootstrap => DetailType::Bootstrap,
            BlockSource::BootstrapLegacy => DetailType::BootstrapLegacy,
            BlockSource::Unchecked => DetailType::Unchecked,
            BlockSource::Local => DetailType::Local,
            BlockSource::Forced => DetailType::Forced,
        }
    }
}

pub struct BlockProcessorContext {
    pub block: Arc<BlockEnum>,
    pub source: BlockSource,
    pub arrival: Instant,
    pub promise: *mut c_void,
}

impl BlockProcessorContext {
    pub fn new(block: Arc<BlockEnum>, source: BlockSource) -> Self {
        Self {
            block,
            source,
            arrival: Instant::now(),
            promise: unsafe {
                CREATE_BLOCK_PROCESSOR_PROMISE.expect("CREATE_BLOCK_PROCESSOR_PROMISE missing")()
            },
        }
    }

    pub fn set_result(&mut self, result: BlockStatus) {
        unsafe {
            BLOCK_PROCESSOR_PROMISE_SET_RESULT.expect("BLOCK_PROCESSOR_PROMISE_SET_RESULT missing")(
                self.promise,
                result as u8,
            );
        }
    }
}

pub static mut CREATE_BLOCK_PROCESSOR_PROMISE: Option<unsafe extern "C" fn() -> *mut c_void> = None;
pub static mut DROP_BLOCK_PROCESSOR_PROMISE: Option<unsafe extern "C" fn(*mut c_void)> = None;
pub static mut BLOCK_PROCESSOR_PROMISE_SET_RESULT: Option<unsafe extern "C" fn(*mut c_void, u8)> =
    None;

impl Drop for BlockProcessorContext {
    fn drop(&mut self) {
        unsafe {
            DROP_BLOCK_PROCESSOR_PROMISE.expect("DROP_BLOCK_PROCESSOR_PROMISE missing")(
                self.promise,
            );
        }
    }
}

pub struct BlockProcessor {
    handle: *mut c_void,
    pub mutex: Mutex<BlockProcessorImpl>,
    pub condition: Condvar,
    pub flushing: AtomicBool,
    pub ledger: Arc<Ledger>,
    pub unchecked_map: Arc<UncheckedMap>,
    config: Arc<NodeConfig>,
    stats: Arc<Stats>,
    work: Arc<WorkThresholds>,
    flags: Arc<NodeFlags>,
    blocks_rolled_back: Mutex<Option<Box<dyn Fn(Vec<BlockEnum>, BlockEnum)>>>,
    block_rolled_back: Mutex<Vec<Box<dyn Fn(&BlockEnum)>>>,
    block_processed: Mutex<Vec<Box<dyn Fn(BlockStatus, &BlockProcessorContext)>>>,
    batch_processed: Mutex<Vec<Box<dyn Fn(&[(BlockStatus, BlockProcessorContext)])>>>,
}

impl BlockProcessor {
    pub fn new(
        handle: *mut c_void,
        config: Arc<NodeConfig>,
        flags: Arc<NodeFlags>,
        ledger: Arc<Ledger>,
        unchecked_map: Arc<UncheckedMap>,
        stats: Arc<Stats>,
        work: Arc<WorkThresholds>,
    ) -> Self {
        let processor_config = config.block_processor.clone();
        let max_size_query = Box::new(move |origin: &Origin<BlockSource>| match origin.source {
            BlockSource::Live => processor_config.max_peer_queue,
            _ => processor_config.max_system_queue,
        });

        let processor_config = config.block_processor.clone();
        let priority_query = Box::new(move |origin: &Origin<BlockSource>| match origin.source {
            BlockSource::Live => processor_config.priority_live,
            BlockSource::Bootstrap | BlockSource::BootstrapLegacy | BlockSource::Unchecked => {
                processor_config.priority_bootstrap
            }
            BlockSource::Local => processor_config.priority_local,
            _ => 1,
        });

        Self {
            handle,
            mutex: Mutex::new(BlockProcessorImpl {
                queue: FairQueue::new(max_size_query, priority_query),
                last_log: None,
                config: Arc::clone(&config),
                stopped: false,
            }),
            condition: Condvar::new(),
            flushing: AtomicBool::new(false),
            ledger,
            unchecked_map,
            config,
            stats,
            work,
            flags,
            blocks_rolled_back: Mutex::new(None),
            block_rolled_back: Mutex::new(Vec::new()),
            block_processed: Mutex::new(Vec::new()),
            batch_processed: Mutex::new(Vec::new()),
        }
    }

    pub fn run(&self) {
        let mut guard = self.mutex.lock().unwrap();
        while !guard.stopped {
            if !guard.queue.is_empty() {
                drop(guard);

                let mut processed = self.process_batch();

                // Set results for futures when not holding the lock
                for (result, context) in processed.iter_mut() {
                    context.set_result(*result);
                }

                self.notify_batch_processed(&processed);

                guard = self.mutex.lock().unwrap();
            } else {
                self.condition.notify_one();
                guard = self.condition.wait(guard).unwrap();
            }
        }
    }

    fn notify_batch_processed(&self, blocks: &Vec<(BlockStatus, BlockProcessorContext)>) {
        {
            let guard = self.block_processed.lock().unwrap();
            for observer in guard.iter() {
                for (status, context) in blocks {
                    observer(*status, context);
                }
            }
        }
        {
            let guard = self.batch_processed.lock().unwrap();
            for observer in guard.iter() {
                observer(&blocks);
            }
        }
    }

    pub fn new_test_instance(ledger: Arc<Ledger>) -> Self {
        BlockProcessor::new(
            std::ptr::null_mut(),
            Arc::new(NodeConfig::new_null()),
            Arc::new(NodeFlags::default()),
            ledger,
            Arc::new(UncheckedMap::default()),
            Arc::new(Stats::default()),
            Arc::new(WORK_THRESHOLDS_STUB.clone()),
        )
    }

    pub fn add_block_processed_observer(
        &self,
        observer: Box<dyn Fn(BlockStatus, &BlockProcessorContext)>,
    ) {
        self.block_processed.lock().unwrap().push(observer);
    }

    pub fn add_batch_processed_observer(
        &self,
        observer: Box<dyn Fn(&[(BlockStatus, BlockProcessorContext)])>,
    ) {
        self.batch_processed.lock().unwrap().push(observer);
    }

    pub fn add_rolled_back_observer(&self, observer: Box<dyn Fn(&BlockEnum)>) {
        self.block_rolled_back.lock().unwrap().push(observer);
    }

    pub fn notify_block_rolled_back(&self, block: &BlockEnum) {
        for observer in self.block_rolled_back.lock().unwrap().iter() {
            observer(block)
        }
    }

    pub fn set_blocks_rolled_back_callback(
        &self,
        callback: Box<dyn Fn(Vec<BlockEnum>, BlockEnum)>,
    ) {
        *self.blocks_rolled_back.lock().unwrap() = Some(callback);
    }

    pub fn process_active(&self, block: Arc<BlockEnum>) {
        unsafe {
            BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK
                .expect("BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK missing")(
                self.handle, block
            )
        }
    }

    pub fn add(
        &self,
        block: Arc<BlockEnum>,
        source: BlockSource,
        channel: Option<Arc<ChannelEnum>>,
    ) -> bool {
        if self.work.validate_entry_block(&block) {
            // true => error
            self.stats.inc(
                StatType::Blockprocessor,
                DetailType::InsufficientWork,
                Direction::In,
            );
            return false; // Not added
        }

        self.stats
            .inc(StatType::Blockprocessor, DetailType::Process, Direction::In);
        debug!(
            "Processing block (async): {} (source: {:?} {})",
            block.hash(),
            source,
            channel
                .as_ref()
                .map(|c| c.remote_endpoint().to_string())
                .unwrap_or_else(|| "<unknown>".to_string())
        );

        self.add_impl(BlockProcessorContext::new(block, source), channel)
    }

    pub fn full(&self) -> bool {
        self.total_queue_len() >= self.flags.block_processor_full_size
    }

    pub fn half_full(&self) -> bool {
        self.total_queue_len() >= self.flags.block_processor_full_size / 2
    }

    // TODO: Remove and replace all checks with calls to size (block_source)
    pub fn total_queue_len(&self) -> usize {
        self.mutex.lock().unwrap().queue.total_len()
    }

    pub fn queue_len(&self, source: BlockSource) -> usize {
        self.mutex.lock().unwrap().queue.len(&source.into())
    }

    pub fn add_impl(
        &self,
        context: BlockProcessorContext,
        channel: Option<Arc<ChannelEnum>>,
    ) -> bool {
        let source = context.source;
        let added;
        {
            let mut guard = self.mutex.lock().unwrap();
            added = guard.queue.push(context, Origin::new_opt(source, channel));
        }
        if added {
            self.condition.notify_all();
        } else {
            self.stats.inc(
                StatType::Blockprocessor,
                DetailType::Overfill,
                Direction::In,
            );
            self.stats.inc(
                StatType::BlockprocessorOverfill,
                source.into(),
                Direction::In,
            );
        }
        added
    }

    pub fn queue_unchecked(&self, hash_or_account: &HashOrAccount) {
        self.unchecked_map.trigger(hash_or_account);
    }

    pub fn process_batch(&self) -> Vec<(BlockStatus, BlockProcessorContext)> {
        let mut processed = Vec::new();

        let _scoped_write_guard = self.ledger.write_queue.wait(Writer::ProcessBatch);
        let mut transaction = self.ledger.rw_txn();
        let mut lock_a = self.mutex.lock().unwrap();

        lock_a.queue.periodic_update(Duration::from_secs(30));

        let timer_l = Instant::now();

        // Processing blocks
        let mut number_of_blocks_processed = 0;
        let mut number_of_forced_processed = 0;

        let deadline_reached = || {
            timer_l.elapsed()
                > Duration::from_millis(self.config.block_processor_batch_max_time_ms as u64)
        };

        while !lock_a.queue.is_empty()
            && (!deadline_reached()
                || number_of_blocks_processed < self.flags.block_processor_batch_size)
        {
            // TODO: Cleaner periodical logging
            if lock_a.should_log() {
                info!(
                    "{} blocks (+ {} forced) in processing queue",
                    lock_a.queue.total_len(),
                    lock_a.queue.len(&BlockSource::Forced.into())
                );
            }
            let context = lock_a.next();
            let force = context.source == BlockSource::Forced;

            drop(lock_a);

            if force {
                number_of_forced_processed += 1;
                self.rollback_competitor(&mut transaction, &context.block);
            }

            number_of_blocks_processed += 1;

            let result = self.process_one(&mut transaction, &context);
            processed.push((result, context));

            lock_a = self.mutex.lock().unwrap();
        }

        drop(lock_a);

        if number_of_blocks_processed != 0 && timer_l.elapsed() > Duration::from_millis(100) {
            debug!(
                "Processed {} blocks ({} blocks were forced) in {} ms",
                number_of_blocks_processed,
                number_of_forced_processed,
                timer_l.elapsed().as_millis(),
            );
        }
        processed
    }

    pub fn process_one(
        &self,
        txn: &mut LmdbWriteTransaction,
        context: &BlockProcessorContext,
    ) -> BlockStatus {
        let block = &context.block;
        let hash = block.hash();
        // this is undefined behaviour and should be fixed ASAP:
        let block_ptr = Arc::as_ptr(block) as *mut BlockEnum;
        let mutable_block = unsafe { &mut *block_ptr };

        let result = match self.ledger.process(txn, mutable_block) {
            Ok(()) => BlockStatus::Progress,
            Err(r) => r,
        };

        self.stats
            .inc(StatType::BlockprocessorResult, result.into(), Direction::In);
        self.stats.inc(
            StatType::BlockprocessorSource,
            context.source.into(),
            Direction::In,
        );
        trace!(?result, block = %block.hash(), source = ?context.source, "Block processed");

        match result {
            BlockStatus::Progress => {
                self.queue_unchecked(&hash.into());
                /* For send blocks check epoch open unchecked (gap pending).
                For state blocks check only send subtype and only if block epoch is not last epoch.
                If epoch is last, then pending entry shouldn't trigger same epoch open block for destination account. */
                if block.block_type() == BlockType::LegacySend
                    || block.block_type() == BlockType::State
                        && block.is_send()
                        && block.sideband().unwrap().details.epoch < Epoch::MAX
                {
                    /* block->destination () for legacy send blocks
                    block->link () for state blocks (send subtype) */
                    self.queue_unchecked(&block.destination_or_link().into());
                }
            }
            BlockStatus::GapPrevious => {
                self.unchecked_map.put(
                    block.previous().into(),
                    UncheckedInfo::new(Arc::clone(block)),
                );
                self.stats
                    .inc(StatType::Ledger, DetailType::GapPrevious, Direction::In);
            }
            BlockStatus::GapSource => {
                self.unchecked_map.put(
                    block
                        .source_field()
                        .unwrap_or(block.link_field().unwrap_or_default().into())
                        .into(),
                    UncheckedInfo::new(Arc::clone(block)),
                );
                self.stats
                    .inc(StatType::Ledger, DetailType::GapSource, Direction::In);
            }
            BlockStatus::GapEpochOpenPending => {
                // Specific unchecked key starting with epoch open block account public key
                self.unchecked_map.put(
                    block.account().into(),
                    UncheckedInfo::new(Arc::clone(block)),
                );
                self.stats
                    .inc(StatType::Ledger, DetailType::GapSource, Direction::In);
            }
            BlockStatus::Old => {
                self.stats
                    .inc(StatType::Ledger, DetailType::Old, Direction::In);
            }
            BlockStatus::BadSignature => {}
            BlockStatus::NegativeSpend => {}
            BlockStatus::Unreceivable => {}
            BlockStatus::Fork => {
                self.stats
                    .inc(StatType::Ledger, DetailType::Fork, Direction::In);
            }
            BlockStatus::OpenedBurnAccount => {}
            BlockStatus::BalanceMismatch => {}
            BlockStatus::RepresentativeMismatch => {}
            BlockStatus::BlockPosition => {}
            BlockStatus::InsufficientWork => {}
        }

        result
    }

    pub fn rollback_competitor(
        &self,
        transaction: &mut LmdbWriteTransaction,
        block: &Arc<BlockEnum>,
    ) {
        let hash = block.hash();
        if let Some(successor) = self
            .ledger
            .successor_by_root(transaction, &block.qualified_root())
        {
            let successor_block = self.ledger.get_block(transaction, &successor).unwrap();
            if successor != hash {
                // Replace our block with the winner and roll back any dependent blocks
                debug!("Rolling back: {} and replacing with: {}", successor, hash);
                let rollback_list = match self.ledger.rollback(transaction, &successor) {
                    Ok(rollback_list) => {
                        self.stats
                            .inc(StatType::Ledger, DetailType::Rollback, Direction::In);
                        debug!("Blocks rolled back: {}", rollback_list.len());
                        rollback_list
                    }
                    Err(_) => {
                        self.stats
                            .inc(StatType::Ledger, DetailType::RollbackFailed, Direction::In);
                        error!(
                            "Failed to roll back: {} because it or a successor was confirmed",
                            successor
                        );
                        Vec::new()
                    }
                };

                let callback_guard = self.blocks_rolled_back.lock().unwrap();
                if let Some(callback) = callback_guard.as_ref() {
                    callback(rollback_list, successor_block);
                }
            }
        }
    }

    pub fn stop(&self) -> std::thread::Result<()> {
        self.mutex.lock().unwrap().stopped = true;
        self.condition.notify_all();
        Ok(())
    }

    pub fn collect_container_info(&self, name: String) -> ContainerInfoComponent {
        let guard = self.mutex.lock().unwrap();
        ContainerInfoComponent::Composite(
            name,
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "blocks".to_owned(),
                    count: guard.queue.total_len(),
                    sizeof_element: size_of::<Arc<BlockEnum>>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "forced".to_owned(),
                    count: guard.queue.len(&BlockSource::Forced.into()),
                    sizeof_element: size_of::<Arc<BlockEnum>>(),
                }),
                guard.queue.collect_container_info("queue"),
            ],
        )
    }
}

unsafe impl Send for BlockProcessor {}
unsafe impl Sync for BlockProcessor {}

pub struct BlockProcessorImpl {
    pub queue: FairQueue<BlockProcessorContext, BlockSource>,
    pub last_log: Option<Instant>,
    config: Arc<NodeConfig>,
    stopped: bool,
}

impl BlockProcessorImpl {
    fn next(&mut self) -> BlockProcessorContext {
        debug_assert!(!self.queue.is_empty()); // This should be checked before calling next
        if !self.queue.is_empty() {
            let (request, origin) = self.queue.next().unwrap();
            assert!(origin.source != BlockSource::Forced || request.source == BlockSource::Forced);
            return request;
        }

        panic!("next() called when no blocks are ready");
    }

    pub fn should_log(&mut self) -> bool {
        if let Some(last) = &self.last_log {
            if last.elapsed() >= Duration::from_secs(15) {
                self.last_log = Some(Instant::now());
                return true;
            }
        }

        false
    }
}
