use std::sync::Arc;

use crate::{
    Account, BlockEnum, BlockHash, Epochs, PublicKey, Signature, SignatureChecker,
    SignatureVerification, SignatureCheckSet,
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
    signature_checker: Arc<SignatureChecker>,
    epochs: Arc<Epochs>,
}

impl<'a> StateBlockSignatureVerification {
    pub fn new(signature_checker: Arc<SignatureChecker>, epochs: Arc<Epochs>) -> Self {
        Self {
            signature_checker,
            epochs,
        }
    }

    pub(crate) fn verify_state_blocks(
        &self,
        items: &[StateBlockSignatureVerificationValue],
    ) -> StateBlockSignatureVerificationResult {
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

        let mut check = SignatureCheckSet{
            messages,
            pub_keys,
            signatures: block_signatures,
            verifications,
        };
        self.signature_checker.verify(&mut check);

        StateBlockSignatureVerificationResult {
            hashes,
            signatures: check.signatures,
            verifications: check.verifications,
        }
    }
}
