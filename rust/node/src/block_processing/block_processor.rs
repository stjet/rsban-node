use super::UncheckedMap;
use crate::{
    config::{NodeConfig, NodeFlags},
    stats::{DetailType, Direction, StatType, Stats},
};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    work::{WorkThresholds, WORK_THRESHOLDS_STUB},
    BlockEnum, BlockType, Epoch, HashOrAccount, UncheckedInfo,
};
use rsnano_ledger::{BlockStatus, Ledger, WriteDatabaseQueue, Writer};
use rsnano_store_lmdb::LmdbWriteTransaction;
use std::{
    collections::VecDeque,
    ffi::c_void,
    mem::size_of,
    sync::{atomic::AtomicBool, Arc, Condvar, Mutex},
    time::{Duration, Instant, SystemTime},
};
use tracing::{debug, error, trace};

pub static mut BLOCKPROCESSOR_ADD_CALLBACK: Option<fn(*mut c_void, Arc<BlockEnum>, BlockSource)> =
    None;
pub static mut BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK: Option<fn(*mut c_void, Arc<BlockEnum>)> =
    None;
pub static mut BLOCKPROCESSOR_HALF_FULL_CALLBACK: Option<
    unsafe extern "C" fn(*mut c_void) -> bool,
> = None;

pub static mut BLOCKPROCESSOR_SIZE_CALLBACK: Option<unsafe extern "C" fn(*mut c_void) -> usize> =
    None;

#[derive(FromPrimitive, Copy, Clone, PartialEq, Eq, Debug)]
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
    pub fn new(block: Arc<BlockEnum>, source: BlockSource, promise: *mut c_void) -> Self {
        Self {
            block,
            source,
            arrival: Instant::now(),
            promise,
        }
    }
}

pub static mut DROP_BLOCK_PROCESSOR_PROMISE: Option<unsafe extern "C" fn(*mut c_void)> = None;

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
    write_database_queue: Arc<WriteDatabaseQueue>,
    flags: Arc<NodeFlags>,
    blocks_rolled_back: Mutex<Option<Box<dyn Fn(Vec<BlockEnum>, BlockEnum)>>>,
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
        write_database_queue: Arc<WriteDatabaseQueue>,
    ) -> Self {
        Self {
            handle,
            mutex: Mutex::new(BlockProcessorImpl {
                blocks: VecDeque::new(),
                forced: VecDeque::new(),
                next_log: SystemTime::now(),
                config: Arc::clone(&config),
            }),
            condition: Condvar::new(),
            flushing: AtomicBool::new(false),
            ledger,
            unchecked_map,
            config,
            stats,
            work,
            write_database_queue,
            flags,
            blocks_rolled_back: Mutex::new(None),
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
            Arc::new(WriteDatabaseQueue::new(false)),
        )
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

    pub fn add(&self, block: Arc<BlockEnum>, source: BlockSource) {
        unsafe {
            BLOCKPROCESSOR_ADD_CALLBACK.expect("BLOCKPROCESSOR_ADD_CALLBACK missing")(
                self.handle,
                block,
                source,
            )
        }
    }

    pub fn half_full(&self) -> bool {
        unsafe {
            BLOCKPROCESSOR_HALF_FULL_CALLBACK.expect("BLOCKPROCESSOR_HALF_FULL_CALLBACK missing")(
                self.handle,
            )
        }
    }

    pub fn queue_len(&self) -> usize {
        unsafe {
            BLOCKPROCESSOR_SIZE_CALLBACK.expect("BLOCKPROCESSOR_SIZE_CALLBACK missing")(self.handle)
        }
    }

    pub fn add_impl(&self, context: BlockProcessorContext) {
        assert_ne!(context.source, BlockSource::Forced);
        {
            let mut lock = self.mutex.lock().unwrap();
            lock.blocks.push_back(context);
        }
        self.condition.notify_all();
    }

    pub fn queue_unchecked(&self, hash_or_account: &HashOrAccount) {
        self.unchecked_map.trigger(hash_or_account);
    }

    pub fn process_batch(&self) -> VecDeque<(BlockStatus, BlockProcessorContext)> {
        let mut processed = VecDeque::new();

        let _scoped_write_guard = self.write_database_queue.wait(Writer::ProcessBatch);
        let mut transaction = self.ledger.rw_txn();
        let mut lock_a = self.mutex.lock().unwrap();
        let timer_l = Instant::now();

        // Processing blocks
        let mut number_of_blocks_processed = 0;
        let mut number_of_forced_processed = 0;

        let deadline_reached = || {
            timer_l.elapsed()
                > Duration::from_millis(self.config.block_processor_batch_max_time_ms as u64)
        };

        while lock_a.have_blocks_ready()
            && (!deadline_reached()
                || number_of_blocks_processed < self.flags.block_processor_batch_size)
        {
            let context = lock_a.next();
            let force = context.source == BlockSource::Forced;

            drop(lock_a);

            if force {
                number_of_forced_processed += 1;
                self.rollback_competitor(&mut transaction, &context.block);
            }

            number_of_blocks_processed += 1;

            let result = self.process_one(&mut transaction, &context);
            processed.push_back((result, context));

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
        if let Some(successor) = self.ledger.successor(transaction, &block.qualified_root()) {
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
        Ok(())
    }

    pub fn collect_container_info(&self, name: String) -> ContainerInfoComponent {
        let (blocks_count, forced_count) = {
            let guard = self.mutex.lock().unwrap();
            (guard.blocks.len(), guard.forced.len())
        };
        ContainerInfoComponent::Composite(
            name,
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "blocks".to_owned(),
                    count: blocks_count,
                    sizeof_element: size_of::<Arc<BlockEnum>>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "forced".to_owned(),
                    count: forced_count,
                    sizeof_element: size_of::<Arc<BlockEnum>>(),
                }),
            ],
        )
    }
}

unsafe impl Send for BlockProcessor {}
unsafe impl Sync for BlockProcessor {}

pub struct BlockProcessorImpl {
    pub blocks: VecDeque<BlockProcessorContext>,
    pub forced: VecDeque<BlockProcessorContext>,
    pub next_log: SystemTime,
    config: Arc<NodeConfig>,
}

impl BlockProcessorImpl {
    pub fn have_blocks_ready(&self) -> bool {
        return self.blocks.len() > 0 || self.forced.len() > 0;
    }

    fn next(&mut self) -> BlockProcessorContext {
        debug_assert!(!self.blocks.is_empty() || !self.forced.is_empty()); // This should be checked before calling next

        if let Some(entry) = self.forced.pop_front() {
            assert_eq!(entry.source, BlockSource::Forced);
            return entry;
        }

        if let Some(entry) = self.blocks.pop_front() {
            assert_ne!(entry.source, BlockSource::Forced);
            return entry;
        }

        panic!("next() called when no blocks are ready");
    }
}
