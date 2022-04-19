use crate::StateBlockSignatureVerification;

use super::{SharedBlockEnumHandle, SignatureCheckerHandle};

pub struct StateBlockSignatureVerificationHandle {
    verification: StateBlockSignatureVerification,
}

#[repr(C)]
pub struct StateBlockSignatureVerificationValueDto {
    pub block: *mut SharedBlockEnumHandle,
    pub account: [u8; 32],
    pub verification: u8,
}

pub struct StateBlockSignatureVerificationResultHandle {
    verifications: Vec<i32>,
    hashes: Vec<[u8; 32]>,
    signatures: Vec<[u8; 64]>,
}

#[repr(C)]
pub struct StateBlockSignatureVerificationResultDto {
    hashes: *const [u8; 32],
    signatures: *const [u8; 64],
    verifications: *const i32,
    size: usize,
    handle: *mut StateBlockSignatureVerificationResultHandle,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_signature_verification_create(
    checker: *const SignatureCheckerHandle,
) -> *mut StateBlockSignatureVerificationHandle {
    let checker = (&*checker).checker.clone();
    Box::into_raw(Box::new(StateBlockSignatureVerificationHandle {
        verification: StateBlockSignatureVerification::new(checker),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_signature_verification_destroy(
    handle: *mut StateBlockSignatureVerificationHandle,
) {
    let bx = Box::from_raw(handle);
    //bx.checker.stop();
    drop(bx);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_signature_verification_verify(
    _handle: &StateBlockSignatureVerificationHandle,
    _items: *const StateBlockSignatureVerificationValueDto,
    _len: usize,
    result: *mut StateBlockSignatureVerificationResultDto,
) {
    //todo: perform verification

    let result_handle = Box::new(StateBlockSignatureVerificationResultHandle {
        verifications: Vec::new(),
        hashes: Vec::new(),
        signatures: Vec::new(),
    });

    let result = &mut *result;
    result.hashes = result_handle.hashes.as_ptr();
    result.signatures = result_handle.signatures.as_ptr();
    result.verifications = result_handle.verifications.as_ptr();
    result.size = 0;
    result.handle = Box::into_raw(result_handle);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_signature_verification_result_destroy(
    handle: *mut StateBlockSignatureVerificationResultHandle,
) {
    drop(Box::from_raw(handle))
}
