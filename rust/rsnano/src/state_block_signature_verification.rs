use std::sync::Arc;

use crate::{SignatureChecker, BlockEnum, Account, SignatureVerification, BlockHash, Signature, PublicKey};

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
}

impl<'a> StateBlockSignatureVerification {
    pub fn new(signature_checker: Arc<SignatureChecker>) -> Self {
        Self { signature_checker }
    }

    pub(crate) fn verify_state_blocks(&self, items: &[StateBlockSignatureVerificationValue]) -> StateBlockSignatureVerificationResult {
        let size = items.len();
        let mut hashes: Vec<BlockHash>= Vec::with_capacity(size);
        let mut messages: Vec<Vec<u8>> = Vec::with_capacity(size);
        let mut accounts: Vec<Account> = Vec::with_capacity(size);
        let mut pub_keys: Vec<PublicKey> = Vec::with_capacity(size);
        let mut block_signatures: Vec<Signature> = Vec::with_capacity(size);
        let mut verifications = vec![0; size];

        for i in items{
            let block = i.block.as_block();
            hashes.push(block.hash());
            messages.push(block.hash().to_bytes().to_vec());
            //  if !block.link().is_zero() && epochs.is_epoch_link(block.link()){
            //      todo!()
            //  }
        }


        StateBlockSignatureVerificationResult { hashes, signatures: block_signatures, verifications }
    }
}