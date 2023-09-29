use std::{
    ffi::{c_char, c_void, CStr},
    ops::{Deref, DerefMut},
    sync::Arc,
};

use rsnano_node::signatures::{
    StateBlockSignatureVerification, StateBlockSignatureVerificationResult,
    StateBlockSignatureVerificationValue,
};

use crate::{
    core::{BlockHandle, EpochsHandle},
    utils::{ContainerInfoComponentHandle, LoggerHandle, LoggerMT},
};

use super::SignatureCheckerHandle;

pub struct StateBlockSignatureVerificationHandle(StateBlockSignatureVerification);

impl Deref for StateBlockSignatureVerificationHandle {
    type Target = StateBlockSignatureVerification;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StateBlockSignatureVerificationHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[repr(C)]
pub struct StateBlockSignatureVerificationValueDto {
    pub block: *mut BlockHandle,
}

#[repr(C)]
pub struct StateBlockSignatureVerificationResultDto {
    hashes: *const [u8; 32],
    signatures: *const [u8; 64],
    verifications: *const i32,
    items: *const StateBlockSignatureVerificationValueDto,
    size: usize,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_signature_verification_create(
    checker: *const SignatureCheckerHandle,
    epochs: *const EpochsHandle,
    logger: *mut LoggerHandle,
    timing_logging: bool,
    verification_size: usize,
) -> *mut StateBlockSignatureVerificationHandle {
    let checker = Arc::clone(&*checker);
    let epochs = Arc::new((*epochs).epochs.clone());
    let logger = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let verification = StateBlockSignatureVerification::builder()
        .signature_checker(checker)
        .epochs(epochs)
        .logger(logger)
        .enable_timing_logging(timing_logging)
        .verification_size(verification_size)
        .spawn()
        .unwrap();
    Box::into_raw(Box::new(StateBlockSignatureVerificationHandle(
        verification,
    )))
}

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_destroy(
    handle: *mut StateBlockSignatureVerificationHandle,
) {
    drop(unsafe { Box::from_raw(handle) });
}

type StateBlockVerifiedCallback =
    unsafe extern "C" fn(*mut c_void, *const StateBlockSignatureVerificationResultDto);

struct ContextHandle(*mut c_void);

impl ContextHandle {
    fn get(&self) -> *mut c_void {
        self.0
    }
}

unsafe impl Send for ContextHandle {}
unsafe impl Sync for ContextHandle {}

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_verified_callback(
    handle: &mut StateBlockSignatureVerificationHandle,
    callback: StateBlockVerifiedCallback,
    context: *mut c_void,
) {
    let context_handle = ContextHandle(context);

    let callback_adapter = Box::new(move |result: StateBlockSignatureVerificationResult| {
        let hashes: Vec<_> = result.hashes.iter().map(|x| *x.as_bytes()).collect();
        let signatures: Vec<_> = result.signatures.iter().map(|x| *x.as_bytes()).collect();
        let items: Vec<_> = result
            .items
            .iter()
            .map(StateBlockSignatureVerificationValueDto::from)
            .collect();

        let result_dto = StateBlockSignatureVerificationResultDto {
            hashes: hashes.as_ptr(),
            signatures: signatures.as_ptr(),
            verifications: result.verifications.as_ptr(),
            size: result.verifications.len(),
            items: items.as_ptr(),
        };

        unsafe {
            (callback)(context_handle.get(), &result_dto);
        }
    });

    handle.set_blocks_verified_callback(callback_adapter);
}

type TransitionInactiveCallback = unsafe extern "C" fn(*mut c_void);

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_transition_inactive_callback(
    handle: &mut StateBlockSignatureVerificationHandle,
    callback: TransitionInactiveCallback,
    context: *mut c_void,
) {
    let context_handle = ContextHandle(context);

    let callback_adapter = Box::new(move || unsafe {
        (callback)(context_handle.get());
    });

    handle.set_transition_inactive_callback(callback_adapter);
}

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_stop(
    handle: &mut StateBlockSignatureVerificationHandle,
) {
    handle.stop().unwrap();
}

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_is_active(
    handle: &StateBlockSignatureVerificationHandle,
) -> bool {
    handle.is_active()
}

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_add(
    handle: &mut StateBlockSignatureVerificationHandle,
    block: &StateBlockSignatureVerificationValueDto,
) {
    let block = StateBlockSignatureVerificationValue {
        block: Arc::clone(unsafe { &*block.block }.deref()),
    };
    handle.add(block);
}

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_size(
    handle: &StateBlockSignatureVerificationHandle,
) -> usize {
    handle.size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_signature_verification_collect_container_info(
    handle: &StateBlockSignatureVerificationHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let name = CStr::from_ptr(name).to_str().unwrap().to_owned();
    Box::into_raw(Box::new(ContainerInfoComponentHandle(
        handle.collect_container_info(name),
    )))
}

impl From<&StateBlockSignatureVerificationValue> for StateBlockSignatureVerificationValueDto {
    fn from(value: &StateBlockSignatureVerificationValue) -> Self {
        StateBlockSignatureVerificationValueDto {
            block: Box::into_raw(Box::new(BlockHandle(value.block.clone()))),
        }
    }
}
