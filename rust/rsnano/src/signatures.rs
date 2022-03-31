use crate::{validate_message_batch, PublicKey, Signature};

pub struct SignatureCheckSet {
    pub messages: Vec<Vec<u8>>,
    pub pub_keys: Vec<PublicKey>,
    pub signatures: Vec<Signature>,
    pub verifications: Vec<i32>,
}

pub struct SignatureCheckSetChunk<'a> {
    pub messages: &'a [Vec<u8>],
    pub pub_keys: &'a [PublicKey],
    pub signatures: &'a [Signature],
    pub verifications: &'a [i32],
}

impl SignatureCheckSet {
    pub fn new(
        messages: Vec<Vec<u8>>,
        pub_keys: Vec<PublicKey>,
        signatures: Vec<Signature>,
    ) -> Self {
        let size = messages.len();
        assert!(pub_keys.len() == size);
        assert!(signatures.len() == size);
        Self {
            messages,
            pub_keys,
            signatures,
            verifications: vec![-1; size],
        }
    }

    pub fn size(&self) -> usize {
        self.messages.len()
    }
}

pub struct SignatureChecker {
    num_threads: usize,
}

impl SignatureChecker {
    pub fn new(num_threads: usize) -> Self {
        Self { num_threads }
    }

    pub const BATCH_SIZE: usize = 256;

    pub fn verify_batch(
        &self,
        check_set: &mut SignatureCheckSet,
        start_index: usize,
        size: usize,
    ) -> bool {
        let range = start_index..start_index + size;
        validate_message_batch(
            &check_set.messages[range.clone()],
            &check_set.pub_keys[range.clone()],
            &check_set.signatures[range.clone()],
            &mut check_set.verifications[range.clone()],
        );

        let valid = &check_set.verifications[range];
        valid.iter().all(|&x| x == 0 || x == 1)
    }

    pub fn verify(&self, check_set: &mut SignatureCheckSet) -> bool {
        if check_set.size() <= SignatureChecker::BATCH_SIZE || self.single_threaded() {
            // Not dealing with many so just use the calling thread for checking signatures
            let result = self.verify_batch(check_set, 0, check_set.size());
            assert!(result);
            return false;
        }

        true
    }

    fn single_threaded(&self) -> bool {
        self.num_threads == 0
    }
}
