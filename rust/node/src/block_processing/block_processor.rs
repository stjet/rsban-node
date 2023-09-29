use rsnano_core::{
    to_hex_string,
    utils::{ContainerInfo, ContainerInfoComponent, Logger},
    work::WorkThresholds,
    BlockEnum, BlockType, Epoch, Epochs, HashOrAccount, UncheckedInfo,
};
use rsnano_ledger::{Ledger, ProcessResult};
use rsnano_store_lmdb::LmdbWriteTransaction;
use std::{
    collections::VecDeque,
    ffi::c_void,
    mem::size_of,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, RwLock,
    },
    time::{Duration, SystemTime},
};

use crate::{
    config::{NodeConfig, NodeFlags},
    signatures::{
        SignatureChecker, StateBlockSignatureVerification, StateBlockSignatureVerificationResult,
        StateBlockSignatureVerificationValue,
    },
    stats::{DetailType, Direction, StatType, Stats},
    unchecked_map::UncheckedMap,
    GapCache,
};

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
    pub state_block_signature_verification: RwLock<StateBlockSignatureVerification>,
    pub flushing: AtomicBool,
    pub ledger: Arc<Ledger>,
    pub unchecked_map: Arc<UncheckedMap>,
    gap_cache: Arc<Mutex<GapCache>>,
    config: Arc<NodeConfig>,
    logger: Arc<dyn Logger>,
    stats: Arc<Stats>,
    work: Arc<WorkThresholds>,
}

impl BlockProcessor {
    pub fn new(
        handle: *mut c_void,
        config: Arc<NodeConfig>,
        signature_checker: Arc<SignatureChecker>,
        epochs: Arc<Epochs>,
        logger: Arc<dyn Logger>,
        flags: Arc<NodeFlags>,
        ledger: Arc<Ledger>,
        unchecked_map: Arc<UncheckedMap>,
        gap_cache: Arc<Mutex<GapCache>>,
        stats: Arc<Stats>,
        work: Arc<WorkThresholds>,
    ) -> Self {
        let state_block_signature_verification = RwLock::new(
            StateBlockSignatureVerification::builder()
                .signature_checker(signature_checker)
                .epochs(epochs)
                .logger(Arc::clone(&logger))
                .enable_timing_logging(config.logging.timing_logging_value)
                .verification_size(flags.block_processor_verification_size)
                .spawn()
                .unwrap(),
        );

        Self {
            handle,
            mutex: Mutex::new(BlockProcessorImpl {
                blocks: VecDeque::new(),
                forced: VecDeque::new(),
                next_log: SystemTime::now(),
                config: Arc::clone(&config),
            }),
            condition: Condvar::new(),
            state_block_signature_verification,
            flushing: AtomicBool::new(false),
            ledger,
            unchecked_map,
            gap_cache,
            config,
            logger,
            stats,
            work,
        }
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
        match block.block_type() {
            BlockType::State | BlockType::LegacyOpen => {
                self.state_block_signature_verification
                    .read()
                    .unwrap()
                    .add(StateBlockSignatureVerificationValue { block });
            }
            _ => {
                {
                    let mut lock = self.mutex.lock().unwrap();
                    lock.blocks.push_back(block);
                }
                self.condition.notify_all();
            }
        }
    }

    pub fn process_verified_state_blocks(&self, mut result: StateBlockSignatureVerificationResult) {
        {
            let mut lk = self.mutex.lock().unwrap();
            for i in 0..result.verifications.len() {
                debug_assert!(result.verifications[i] == 1 || result.verifications[i] == 0);
                let block = result.items.pop_front().unwrap();
                if !block.block.link().is_zero() && self.ledger.is_epoch_link(&block.block.link()) {
                    // Epoch block or possible regular state blocks with epoch link (send subtype)
                    lk.blocks.push_back(block.block);
                } else if result.verifications[i] == 1 {
                    // Non epoch blocks
                    lk.blocks.push_back(block.block);
                }
            }
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

    pub fn process_one(
        &self,
        txn: &mut LmdbWriteTransaction,
        block: &Arc<BlockEnum>,
    ) -> ProcessResult {
        let hash = block.hash();

        // this is undefined behaviour and should be fixed ASAP:
        let block_ptr = Arc::as_ptr(block) as *mut BlockEnum;
        let mutable_block = unsafe { &mut *block_ptr };

        let result = self.ledger.process(txn, mutable_block);
        match result {
            Ok(()) => {
                if self.config.logging.ledger_logging_value {
                    let block_string = block.to_json().unwrap();
                    self.logger
                        .try_log(&format!("Processing block {}: {}", hash, block_string));
                }
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
            Err(ProcessResult::GapPrevious) => {
                if self.config.logging.ledger_logging_value {
                    self.logger.try_log(&format!("Gap previous for: {hash}"));
                }
                self.unchecked_map.put(
                    block.previous().into(),
                    UncheckedInfo::new(Arc::clone(block)),
                );
                self.stats
                    .inc(StatType::Ledger, DetailType::GapPrevious, Direction::In);
            }
            Err(ProcessResult::GapSource) => {
                if self.config.logging.ledger_logging_value {
                    self.logger.try_log(&format!("Gap source for: {hash}"));
                }
                self.unchecked_map.put(
                    self.ledger.block_source(txn, block).into(),
                    UncheckedInfo::new(Arc::clone(block)),
                );
                self.stats
                    .inc(StatType::Ledger, DetailType::GapSource, Direction::In);
            }
            Err(ProcessResult::GapEpochOpenPending) => {
                if self.config.logging.ledger_logging_value {
                    self.logger
                        .try_log(&format!("Gap pending entries for epoch open: {hash}"));
                }
                // Specific unchecked key starting with epoch open block account public key
                self.unchecked_map.put(
                    block.account_calculated().into(),
                    UncheckedInfo::new(Arc::clone(block)),
                );
                self.stats
                    .inc(StatType::Ledger, DetailType::GapSource, Direction::In);
            }
            Err(ProcessResult::Old) => {
                if self.config.logging.ledger_duplicate_logging() {
                    self.logger.try_log(&format!("Old for: {hash}"));
                }
                self.stats
                    .inc(StatType::Ledger, DetailType::Old, Direction::In);
            }
            Err(ProcessResult::BadSignature) => {
                if self.config.logging.ledger_logging_value {
                    self.logger.try_log(&format!("Bad signature for: {hash}"));
                }
            }
            Err(ProcessResult::NegativeSpend) => {
                if self.config.logging.ledger_logging_value {
                    self.logger.try_log(&format!("Negative spend for: {hash}"));
                }
            }
            Err(ProcessResult::Unreceivable) => {
                if self.config.logging.ledger_logging_value {
                    self.logger.try_log(&format!("Unreceivable for: {hash}"));
                }
            }
            Err(ProcessResult::Fork) => {
                self.stats
                    .inc(StatType::Ledger, DetailType::Fork, Direction::In);

                if self.config.logging.ledger_logging_value {
                    self.logger
                        .try_log(&format!("Fork for: {hash} root: {}", block.root()));
                }
            }
            Err(ProcessResult::OpenedBurnAccount) => {
                if self.config.logging.ledger_logging_value {
                    self.logger
                        .try_log(&format!("Rejecting open block for burn account: {hash}"));
                }
            }
            Err(ProcessResult::BalanceMismatch) => {
                if self.config.logging.ledger_logging_value {
                    self.logger
                        .try_log(&format!("Balance mismatch for: {hash}"));
                }
            }
            Err(ProcessResult::RepresentativeMismatch) => {
                if self.config.logging.ledger_logging_value {
                    self.logger
                        .try_log(&format!("Representative mismatch for: {hash}"));
                }
            }
            Err(ProcessResult::BlockPosition) => {
                if self.config.logging.ledger_logging_value {
                    self.logger.try_log(&format!(
                        "Block {hash} cannot follow predecessor: {}",
                        block.previous()
                    ));
                }
            }
            Err(ProcessResult::InsufficientWork) => {
                if self.config.logging.ledger_logging_value {
                    self.logger.try_log(&format!(
                        "Insufficient work for {hash} : {} (difficulty {})",
                        to_hex_string(block.work()),
                        to_hex_string(self.work.difficulty_block(block))
                    ));
                }
            }
            Err(ProcessResult::Progress) => {
                unreachable!()
            }
        }

        let result = match result {
            Ok(()) => ProcessResult::Progress,
            Err(r) => r,
        };
        self.stats
            .inc(StatType::Blockprocessor, result.into(), Direction::In);
        result
    }

    pub fn stop(&self) -> std::thread::Result<()> {
        self.state_block_signature_verification
            .write()
            .unwrap()
            .stop()
    }

    pub fn collect_container_info(&self, name: String) -> ContainerInfoComponent {
        let (blocks_count, forced_count) = {
            let guard = self.mutex.lock().unwrap();
            (guard.blocks.len(), guard.forced.len())
        };
        ContainerInfoComponent::Composite(
            name,
            vec![
                self.state_block_signature_verification
                    .read()
                    .unwrap()
                    .collect_container_info("state_block_signature_verification"),
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

pub trait BlockProcessorExt {
    fn init(&self);
}

impl BlockProcessorExt for Arc<BlockProcessor> {
    fn init(&self) {
        let self_weak = Arc::downgrade(&self);
        let lock = self.state_block_signature_verification.read().unwrap();
        lock.set_blocks_verified_callback(Box::new(move |result| {
            if let Some(processor) = self_weak.upgrade() {
                processor.process_verified_state_blocks(result);
            }
        }));

        let self_weak = Arc::downgrade(&self);
        lock.set_transition_inactive_callback(Box::new(move || {
            if let Some(processor) = self_weak.upgrade() {
                if processor.flushing.load(Ordering::SeqCst) {
                    {
                        // Prevent a race with condition.wait in block_processor::flush
                        let _guard = processor.mutex.lock().unwrap();
                    }
                    processor.condition.notify_all();
                }
            }
        }))
    }
}
