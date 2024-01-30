use rsnano_core::{
    to_hex_string,
    utils::{ContainerInfo, ContainerInfoComponent, LogType, Logger},
    work::WorkThresholds,
    BlockEnum, BlockType, Epoch, HashOrAccount, UncheckedInfo,
};
use rsnano_ledger::{Ledger, ProcessResult, WriteDatabaseQueue, Writer};
use rsnano_store_lmdb::LmdbWriteTransaction;
use std::{
    collections::VecDeque,
    ffi::c_void,
    mem::size_of,
    sync::{atomic::AtomicBool, Arc, Condvar, Mutex},
    time::{Duration, Instant, SystemTime},
};

use crate::{
    config::{NodeConfig, NodeFlags},
    stats::{DetailType, Direction, StatType, Stats},
};

use super::{GapCache, UncheckedMap};

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
    pub flushing: AtomicBool,
    pub ledger: Arc<Ledger>,
    pub unchecked_map: Arc<UncheckedMap>,
    gap_cache: Arc<Mutex<GapCache>>,
    config: Arc<NodeConfig>,
    logger: Arc<dyn Logger>,
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
        logger: Arc<dyn Logger>,
        flags: Arc<NodeFlags>,
        ledger: Arc<Ledger>,
        unchecked_map: Arc<UncheckedMap>,
        gap_cache: Arc<Mutex<GapCache>>,
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
            gap_cache,
            config,
            logger,
            stats,
            work,
            write_database_queue,
            flags,
            blocks_rolled_back: Mutex::new(None),
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

    pub fn add_impl(&self, block: Arc<BlockEnum>) {
        {
            let mut lock = self.mutex.lock().unwrap();
            lock.blocks.push_back(block);
        }
        self.condition.notify_all();
    }

    pub fn queue_unchecked(&self, hash_or_account: &HashOrAccount) {
        self.unchecked_map.trigger(hash_or_account);
        self.gap_cache
            .lock()
            .unwrap()
            .erase(&hash_or_account.into())
    }

    pub fn process_batch(&self) -> VecDeque<(ProcessResult, Arc<BlockEnum>)> {
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
            let block: Arc<BlockEnum>;
            let force: bool;
            if lock_a.forced.len() == 0 {
                block = lock_a.blocks.pop_front().unwrap();
                force = false;
            } else {
                block = lock_a.forced.pop_front().unwrap();
                force = true;
                number_of_forced_processed += 1;
            }
            drop(lock_a);

            if force {
                self.rollback_competitor(&mut transaction, &block);
            }
            number_of_blocks_processed += 1;
            let result = self.process_one(&mut transaction, &block);
            processed.push_back((result, block));
            lock_a = self.mutex.lock().unwrap();
        }
        drop(lock_a);

        if self.config.logging.timing_logging_value
            && number_of_blocks_processed != 0
            && timer_l.elapsed() > Duration::from_millis(100)
        {
            self.logger.debug(
                LogType::Blockprocessor,
                &format!(
                    "Processed {} blocks ({} blocks were forced) in {} ms",
                    number_of_blocks_processed,
                    number_of_forced_processed,
                    timer_l.elapsed().as_millis(),
                ),
            );
        }
        processed
    }

    pub fn process_one(
        &self,
        txn: &mut LmdbWriteTransaction,
        block: &Arc<BlockEnum>,
    ) -> ProcessResult {
        let hash = block.hash();

        // this is undefined behaviour and should be fixed ASAP:
        let block_ptr = Arc::as_ptr(block) as *mut BlockEnum;
        let mutable_block = unsafe { &mut *block_ptr };

        let result = match self.ledger.process(txn, mutable_block) {
            Ok(()) => ProcessResult::Progress,
            Err(r) => r,
        };

        self.stats
            .inc(StatType::Blockprocessor, result.into(), Direction::In);

        match result {
            ProcessResult::Progress => {
                self.queue_unchecked(&hash.into());
                /* For send blocks check epoch open unchecked (gap pending).
                For state blocks check only send subtype and only if block epoch is not last epoch.
                If epoch is last, then pending entry shouldn't trigger same epoch open block for destination account. */
                if block.block_type() == BlockType::LegacySend
                    || block.block_type() == BlockType::State
                        && block.sideband().unwrap().details.is_send
                        && block.sideband().unwrap().details.epoch < Epoch::MAX
                {
                    /* block->destination () for legacy send blocks
                    block->link () for state blocks (send subtype) */
                    self.queue_unchecked(&block.destination_or_link().into());
                }
            }
            ProcessResult::GapPrevious => {
                self.unchecked_map.put(
                    block.previous().into(),
                    UncheckedInfo::new(Arc::clone(block)),
                );
                self.stats
                    .inc(StatType::Ledger, DetailType::GapPrevious, Direction::In);
            }
            ProcessResult::GapSource => {
                self.unchecked_map.put(
                    self.ledger.block_source(txn, block).into(),
                    UncheckedInfo::new(Arc::clone(block)),
                );
                self.stats
                    .inc(StatType::Ledger, DetailType::GapSource, Direction::In);
            }
            ProcessResult::GapEpochOpenPending => {
                // Specific unchecked key starting with epoch open block account public key
                self.unchecked_map.put(
                    block.account_calculated().into(),
                    UncheckedInfo::new(Arc::clone(block)),
                );
                self.stats
                    .inc(StatType::Ledger, DetailType::GapSource, Direction::In);
            }
            ProcessResult::Old => {
                self.stats
                    .inc(StatType::Ledger, DetailType::Old, Direction::In);
            }
            ProcessResult::BadSignature => {}
            ProcessResult::NegativeSpend => {}
            ProcessResult::Unreceivable => {}
            ProcessResult::Fork => {
                self.stats
                    .inc(StatType::Ledger, DetailType::Fork, Direction::In);
            }
            ProcessResult::OpenedBurnAccount => {}
            ProcessResult::BalanceMismatch => {}
            ProcessResult::RepresentativeMismatch => {}
            ProcessResult::BlockPosition => {}
            ProcessResult::InsufficientWork => {}
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
            if successor.hash() != hash {
                // Replace our block with the winner and roll back any dependent blocks
                self.logger.debug(
                    LogType::Blockprocessor,
                    &format!(
                        "Rolling back: {} and replacing with: {}",
                        successor.hash(),
                        hash
                    ),
                );
                let rollback_list = match self.ledger.rollback(transaction, &successor.hash()) {
                    Ok(rollback_list) => {
                        self.logger.debug(
                            LogType::Blockprocessor,
                            &format!("Blocks rolled back: {}", rollback_list.len()),
                        );
                        rollback_list
                    }
                    Err(_) => {
                        self.stats
                            .inc(StatType::Ledger, DetailType::RollbackFailed, Direction::In);
                        self.logger.error(
                            LogType::Blockprocessor,
                            &format!(
                                "Failed to roll back: {} because it or a successor was confirmed",
                                successor.hash()
                            ),
                        );
                        Vec::new()
                    }
                };

                let callback_guard = self.blocks_rolled_back.lock().unwrap();
                if let Some(callback) = callback_guard.as_ref() {
                    callback(rollback_list, successor);
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
    pub blocks: VecDeque<Arc<BlockEnum>>,
    pub forced: VecDeque<Arc<BlockEnum>>,
    pub next_log: SystemTime,
    config: Arc<NodeConfig>,
}

impl BlockProcessorImpl {
    pub fn have_blocks_ready(&self) -> bool {
        return self.blocks.len() > 0 || self.forced.len() > 0;
    }

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
