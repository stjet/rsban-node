use std::{
    any::Any,
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, RwLock,
    },
    thread::JoinHandle,
    time::Duration,
};

use crate::{
    Account, BlockEnum, BlockHash, Epochs, Logger, PublicKey, Signature, SignatureCheckSet,
    SignatureChecker, SignatureVerification,
};

pub(crate) struct StateBlockSignatureVerificationValue {
    pub block: Arc<RwLock<BlockEnum>>,
    pub account: Account,
    pub verification: SignatureVerification,
}

pub(crate) struct StateBlockSignatureVerificationResult {
    pub hashes: Vec<BlockHash>,
    pub signatures: Vec<Signature>,
    pub verifications: Vec<i32>,
    pub items: VecDeque<StateBlockSignatureVerificationValue>,
}

pub(crate) struct StateBlockSignatureVerification {
    join_handle: Option<JoinHandle<()>>,
    thread: Arc<StateBlockSignatureVerificationThread>,
}

impl<'a> StateBlockSignatureVerification {
    pub fn new(
        signature_checker: Arc<SignatureChecker>,
        epochs: Arc<Epochs>,
        logger: Arc<dyn Logger>,
        verification_size: usize,
    ) -> std::io::Result<Self> {
        let thread = Arc::new(StateBlockSignatureVerificationThread::new(
            verification_size,
            signature_checker,
            epochs,
            logger,
        ));

        let thread_clone = thread.clone();
        let join_handle = std::thread::Builder::new()
            .name("State block sig".to_string())
            .spawn(move || {
                thread_clone.run();
            })?;

        Ok(Self {
            join_handle: Some(join_handle),
            thread,
        })
    }

    pub(crate) fn set_blocks_verified_callback(
        &mut self,
        callback: fn(&dyn Any, StateBlockSignatureVerificationResult),
        context: Box<dyn Any + Send + Sync>,
    ) {
        let mut lk = self.thread.callbacks.lock().unwrap();
        lk.blocks_verified_callback = callback;
        lk.blocks_verified_callback_context = Some(context);
    }

    pub(crate) fn set_transition_inactive_callback(
        &mut self,
        callback: fn(&dyn Any),
        context: Box<dyn Any + Send + Sync>,
    ) {
        let mut lk = self.thread.callbacks.lock().unwrap();
        lk.transition_inactive_callback = callback;
        lk.transition_inactive_callback_context = Some(context);
    }

    pub(crate) fn stop(&mut self) -> std::thread::Result<()> {
        {
            let mut lk = self.thread.mutable.lock().unwrap();
            lk.stopped = true;
        }

        if let Some(handle) = self.join_handle.take() {
            self.thread.condition.notify_one();
            handle.join()?;
        }
        Ok(())
    }

    pub(crate) fn add(&self, block: StateBlockSignatureVerificationValue) {
        {
            let mut lk = self.thread.mutable.lock().unwrap();
            lk.state_blocks.push_back(block);
        }
        self.thread.condition.notify_one();
    }

    pub(crate) fn size(&self) -> usize {
        let lk = self.thread.mutable.lock().unwrap();
        lk.state_blocks.len()
    }

    pub(crate) fn is_active(&self) -> bool {
        let lk = self.thread.mutable.lock().unwrap();
        lk.active
    }

    pub(crate) fn enable_timing_logging(&self, enable: bool) {
        self.thread.timing_logging.store(enable, Ordering::Relaxed);
    }
}

impl Drop for StateBlockSignatureVerification {
    fn drop(&mut self) {
        self.stop()
            .expect("Could not stop state block verification thread");
    }
}

struct StateBlockSignatureVerificationThread {
    condition: Condvar,
    verification_size: usize,
    signature_checker: Arc<SignatureChecker>,
    epochs: Arc<Epochs>,
    logger: Arc<dyn Logger>,
    mutable: Mutex<ThreadMutableData>,
    callbacks: Mutex<Callbacks>,
    timing_logging: AtomicBool,
}

struct ThreadMutableData {
    state_blocks: VecDeque<StateBlockSignatureVerificationValue>,
    active: bool,
    stopped: bool,
}

struct Callbacks {
    blocks_verified_callback: fn(&dyn Any, StateBlockSignatureVerificationResult),
    blocks_verified_callback_context: Option<Box<dyn Any + Send + Sync>>,
    transition_inactive_callback: fn(&dyn Any) -> (),
    transition_inactive_callback_context: Option<Box<dyn Any + Send + Sync>>,
}

impl StateBlockSignatureVerificationThread {
    fn new(
        verification_size: usize,
        signature_checker: Arc<SignatureChecker>,
        epochs: Arc<Epochs>,
        logger: Arc<dyn Logger>,
    ) -> Self {
        Self {
            condition: Condvar::new(),
            verification_size,
            signature_checker,
            epochs,
            logger,
            timing_logging: AtomicBool::new(false),
            mutable: Mutex::new(ThreadMutableData {
                state_blocks: VecDeque::new(),
                active: false,
                stopped: false,
            }),
            callbacks: Mutex::new(Callbacks {
                blocks_verified_callback: |_, _| {},
                blocks_verified_callback_context: None,
                transition_inactive_callback: |_| {},
                transition_inactive_callback_context: None,
            }),
        }
    }

    fn run(&self) {
        let mut lk = self.mutable.lock().unwrap();
        while !lk.stopped {
            if !lk.state_blocks.is_empty() {
                let max_verification_batch = if self.verification_size != 0 {
                    self.verification_size
                } else {
                    self.signature_checker.max_size()
                };
                lk.active = true;
                while !lk.state_blocks.is_empty() && !lk.stopped {
                    let items = Self::setup_items(&mut lk, max_verification_batch);
                    drop(lk);
                    self.verify_state_blocks(items);
                    lk = self.mutable.lock().unwrap();
                }
                lk.active = false;
                drop(lk);
                {
                    let callback_lk = self.callbacks.lock().unwrap();
                    if let Some(context) = &callback_lk.transition_inactive_callback_context {
                        (callback_lk.transition_inactive_callback)(context.as_ref());
                    }
                }
                lk = self.mutable.lock().unwrap();
            } else {
                lk = self.condition.wait(lk).unwrap();
            }
        }
    }

    fn setup_items(
        data: &mut ThreadMutableData,
        max_count: usize,
    ) -> VecDeque<StateBlockSignatureVerificationValue> {
        let mut items = VecDeque::new();
        if data.state_blocks.len() <= max_count {
            std::mem::swap(&mut items, &mut data.state_blocks);
        } else {
            for _ in 0..max_count {
                if let Some(item) = data.state_blocks.pop_front() {
                    items.push_back(item);
                }
            }
            debug_assert!(!data.state_blocks.is_empty());
        }

        items
    }

    fn verify_state_blocks(&self, items: VecDeque<StateBlockSignatureVerificationValue>) {
        if items.is_empty() {
            return;
        }

        let now = std::time::Instant::now();
        let size = items.len();
        let mut hashes: Vec<BlockHash> = Vec::with_capacity(size);
        let mut messages: Vec<Vec<u8>> = Vec::with_capacity(size);
        let mut accounts: Vec<Account> = Vec::with_capacity(size);
        let mut pub_keys: Vec<PublicKey> = Vec::with_capacity(size);
        let mut block_signatures: Vec<Signature> = Vec::with_capacity(size);
        let verifications = vec![0; size];

        for i in &items {
            let guard = i.block.read().unwrap();
            let block = guard.as_block();
            hashes.push(block.hash());
            messages.push(block.hash().to_bytes().to_vec());
            let mut account_l = *block.account();
            if !block.link().is_zero() && self.epochs.is_epoch_link(&block.link()) {
                account_l = self
                    .epochs
                    .signer(self.epochs.epoch(&block.link()).unwrap())
                    .unwrap()
                    .into();
            } else if !i.account.is_zero() {
                account_l = i.account;
            }
            accounts.push(account_l);
            pub_keys.push(account_l.public_key);
            block_signatures.push(block.block_signature().clone())
        }

        let mut check = SignatureCheckSet {
            messages,
            pub_keys,
            signatures: block_signatures,
            verifications,
        };
        self.signature_checker.verify(&mut check);

        if self.timing_logging.load(Ordering::Relaxed) && now.elapsed() > Duration::from_millis(10)
        {
            self.logger.try_log(&format!(
                "Batch verified {} state blocks in {} ms",
                size,
                now.elapsed().as_millis()
            ));
        }

        let result = StateBlockSignatureVerificationResult {
            hashes,
            signatures: check.signatures,
            verifications: check.verifications,
            items,
        };

        let lk = self.callbacks.lock().unwrap();
        if let Some(ctx) = &lk.blocks_verified_callback_context {
            (lk.blocks_verified_callback)(ctx.as_ref(), result);
        }
    }
}
