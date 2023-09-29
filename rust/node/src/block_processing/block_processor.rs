use rsnano_core::{utils::Logger, BlockEnum, BlockType, Epochs};
use rsnano_ledger::Ledger;
use std::{
    collections::VecDeque,
    ffi::c_void,
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
}

impl BlockProcessor {
    pub fn new(
        handle: *mut c_void,
        config: NodeConfig,
        signature_checker: Arc<SignatureChecker>,
        epochs: Arc<Epochs>,
        logger: Arc<dyn Logger>,
        flags: Arc<NodeFlags>,
        ledger: Arc<Ledger>,
    ) -> Self {
        let state_block_signature_verification = RwLock::new(
            StateBlockSignatureVerification::builder()
                .signature_checker(signature_checker)
                .epochs(epochs)
                .logger(logger)
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
                config,
            }),
            condition: Condvar::new(),
            state_block_signature_verification,
            flushing: AtomicBool::new(false),
            ledger,
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

    pub fn stop(&self) -> std::thread::Result<()> {
        self.state_block_signature_verification
            .write()
            .unwrap()
            .stop()
    }
}

unsafe impl Send for BlockProcessor {}
unsafe impl Sync for BlockProcessor {}

pub struct BlockProcessorImpl {
    pub blocks: VecDeque<Arc<BlockEnum>>,
    pub forced: VecDeque<Arc<BlockEnum>>,
    pub next_log: SystemTime,
    config: NodeConfig,
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
        let self_clone = Arc::clone(&self);
        let lock = self.state_block_signature_verification.read().unwrap();
        lock.set_blocks_verified_callback(Box::new(move |result| {
            self_clone.process_verified_state_blocks(result);
        }));

        let self_clone = Arc::clone(&self);
        lock.set_transition_inactive_callback(Box::new(move || {
            if self_clone.flushing.load(Ordering::SeqCst) {
                {
                    // Prevent a race with condition.wait in block_processor::flush
                    let _guard = self_clone.mutex.lock().unwrap();
                }
                self_clone.condition.notify_all();
            }
        }))
    }
}
