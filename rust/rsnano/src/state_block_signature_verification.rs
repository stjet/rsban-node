use std::{
    any::Any,
    collections::VecDeque,
    sync::{Arc, Mutex, RwLock},
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
    pub items: Vec<StateBlockSignatureVerificationValue>,
}

pub(crate) struct StateBlockSignatureVerification {
    pub timing_logging: bool,
    blocks_verified_callback: fn(&dyn Any, StateBlockSignatureVerificationResult),
    blocks_verified_callback_context: Option<Box<dyn Any>>,
    signature_checker: Arc<SignatureChecker>,
    epochs: Arc<Epochs>,
    logger: Arc<dyn Logger>,
    //todo remove pub
    pub state_blocks: Mutex<VecDeque<StateBlockSignatureVerificationValue>>,
    pub active: bool,
    pub stopped: bool,
}

impl<'a> StateBlockSignatureVerification {
    pub fn new(
        signature_checker: Arc<SignatureChecker>,
        epochs: Arc<Epochs>,
        logger: Arc<dyn Logger>,
    ) -> Self {
        Self {
            active: false,
            stopped: false,
            signature_checker,
            epochs,
            timing_logging: false,
            logger,
            blocks_verified_callback: |_, _| {},
            blocks_verified_callback_context: None,
            state_blocks: Mutex::new(VecDeque::new()),
        }
    }

    pub(crate) fn set_blocks_verified_callback(
        &mut self,
        callback: fn(&dyn Any, StateBlockSignatureVerificationResult),
        context: Box<dyn Any>,
    ) {
        self.blocks_verified_callback = callback;
        self.blocks_verified_callback_context = Some(context);
    }

    pub(crate) fn verify_state_blocks(&self, items: Vec<StateBlockSignatureVerificationValue>) {
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

        if self.timing_logging && now.elapsed() > Duration::from_millis(10) {
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

        if let Some(ctx) = &self.blocks_verified_callback_context {
            (self.blocks_verified_callback)(ctx.as_ref(), result);
        }
    }

    pub(crate) fn setup_items(
        &self,
        max_count: usize,
    ) -> VecDeque<StateBlockSignatureVerificationValue> {
        let mut items = VecDeque::new();
        let mut blocks = self.state_blocks.lock().unwrap();
        if blocks.len() <= max_count {
            std::mem::swap(&mut items, &mut blocks);
        } else {
            for _ in 0..max_count {
                if let Some(item) = blocks.pop_front() {
                    items.push_back(item);
                }
            }
            debug_assert!(!blocks.is_empty());
        }

        items
    }
}
