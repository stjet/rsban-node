use std::{sync::Arc, time::Duration};

use crate::{
    Account, BlockEnum, BlockHash, Epochs, Logger, PublicKey, Signature, SignatureCheckSet,
    SignatureChecker, SignatureVerification,
};

pub(crate) struct StateBlockSignatureVerificationValue {
    pub block: Arc<BlockEnum>,
    pub account: Account,
    pub verification: SignatureVerification,
}

pub(crate) struct StateBlockSignatureVerificationResult {
    pub hashes: Vec<BlockHash>,
    pub signatures: Vec<Signature>,
    pub verifications: Vec<i32>,
}

pub(crate) struct StateBlockSignatureVerification {
    pub timing_logging: bool,
    signature_checker: Arc<SignatureChecker>,
    epochs: Arc<Epochs>,
    logger: Arc<dyn Logger>,
}

impl<'a> StateBlockSignatureVerification {
    pub fn new(
        signature_checker: Arc<SignatureChecker>,
        epochs: Arc<Epochs>,
        logger: Arc<dyn Logger>,
    ) -> Self {
        Self {
            signature_checker,
            epochs,
            timing_logging: false,
            logger,
        }
    }

    pub(crate) fn verify_state_blocks(
        &self,
        items: &[StateBlockSignatureVerificationValue],
    ) -> Option<StateBlockSignatureVerificationResult> {
        if items.is_empty() {
            return None;
        }

        let now = std::time::Instant::now();
        let size = items.len();
        let mut hashes: Vec<BlockHash> = Vec::with_capacity(size);
        let mut messages: Vec<Vec<u8>> = Vec::with_capacity(size);
        let mut accounts: Vec<Account> = Vec::with_capacity(size);
        let mut pub_keys: Vec<PublicKey> = Vec::with_capacity(size);
        let mut block_signatures: Vec<Signature> = Vec::with_capacity(size);
        let verifications = vec![0; size];

        for i in items {
            let block = i.block.as_block();
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

        Some(StateBlockSignatureVerificationResult {
            hashes,
            signatures: check.signatures,
            verifications: check.verifications,
        })
    }
}
