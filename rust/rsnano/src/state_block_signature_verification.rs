use std::sync::Arc;

use crate::SignatureChecker;

pub(crate) struct StateBlockSignatureVerification {
    signature_checker: Arc<SignatureChecker>,
}

impl<'a> StateBlockSignatureVerification {
    pub fn new(signature_checker: Arc<SignatureChecker>) -> Self {
        Self { signature_checker }
    }
}
