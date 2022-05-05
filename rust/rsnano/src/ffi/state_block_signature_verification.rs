use std::{any::Any, ffi::c_void, sync::Arc};

use num::FromPrimitive;

use crate::{
    state_block_signature_verification::{
        StateBlockSignatureVerificationResult, StateBlockSignatureVerificationValue,
    },
    Account, StateBlockSignatureVerification,
};

use super::{BlockHandle, EpochsHandle, LoggerMT, SignatureCheckerHandle};

pub struct StateBlockSignatureVerificationHandle {
    verification: StateBlockSignatureVerification,
}

#[repr(C)]
pub struct StateBlockSignatureVerificationValueDto {
    pub block: *mut BlockHandle,
    pub account: [u8; 32],
    pub verification: u8,
}

pub struct StateBlockSignatureVerificationResultHandle {
    verifications: Vec<i32>,
    hashes: Vec<[u8; 32]>,
    signatures: Vec<[u8; 64]>,
    items: Vec<StateBlockSignatureVerificationValueDto>,
}

#[repr(C)]
pub struct StateBlockSignatureVerificationResultDto {
    hashes: *const [u8; 32],
    signatures: *const [u8; 64],
    verifications: *const i32,
    items: *const StateBlockSignatureVerificationValueDto,
    size: usize,
    handle: *mut StateBlockSignatureVerificationResultHandle,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_signature_verification_create(
    checker: *const SignatureCheckerHandle,
    epochs: *const EpochsHandle,
    logger: *mut c_void,
    timing_logging: bool,
) -> *mut StateBlockSignatureVerificationHandle {
    let checker = (*checker).checker.clone();
    let epochs = Arc::new((*epochs).epochs.clone());
    let logger = Arc::new(LoggerMT::new(logger));
    let mut verification = StateBlockSignatureVerification::new(checker, epochs, logger);
    verification.timing_logging = timing_logging;
    Box::into_raw(Box::new(StateBlockSignatureVerificationHandle {
        verification,
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
    handle: &StateBlockSignatureVerificationHandle,
    items: *const StateBlockSignatureVerificationValueDto,
    len: usize,
) {
    let items = std::slice::from_raw_parts(items, len);
    let items: Vec<_> = items
        .iter()
        .map(|i| StateBlockSignatureVerificationValue {
            block: (*i.block).block.clone(),
            account: crate::Account::from_bytes(i.account),
            verification: FromPrimitive::from_u8(i.verification).unwrap(),
        })
        .collect();

    handle.verification.verify_state_blocks(items);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_signature_verification_result_destroy(
    handle: *mut StateBlockSignatureVerificationResultHandle,
) {
    drop(Box::from_raw(handle))
}

type StateBlockVerifiedCallback =
    unsafe extern "C" fn(*mut c_void, *const StateBlockSignatureVerificationResultDto);

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_signature_verification_verified_callback(
    handle: *mut StateBlockSignatureVerificationHandle,
    callback: StateBlockVerifiedCallback,
    context: *mut c_void,
) {
    let handle = &mut *handle;
    let context = Box::new(StateBlocksVerifiedContext {
        ffi_context: context,
        ffi_callback: callback,
    });
    handle
        .verification
        .set_blocks_verified_callback(blocks_verified_callback_adapter, context);
}

struct StateBlocksVerifiedContext {
    pub ffi_context: *mut c_void,
    pub ffi_callback: StateBlockVerifiedCallback,
}

fn blocks_verified_callback_adapter(
    context: &dyn Any,
    result: StateBlockSignatureVerificationResult,
) {
    let result_handle = Box::new(StateBlockSignatureVerificationResultHandle {
        verifications: result.verifications,
        hashes: result.hashes.iter().map(|x| x.to_bytes()).collect(),
        signatures: result.signatures.iter().map(|x| *x.as_bytes()).collect(),
        items: result
            .items
            .iter()
            .map(StateBlockSignatureVerificationValueDto::from)
            .collect(),
    });

    let result_dto = StateBlockSignatureVerificationResultDto {
        hashes: result_handle.hashes.as_ptr(),
        signatures: result_handle.signatures.as_ptr(),
        verifications: result_handle.verifications.as_ptr(),
        size: result_handle.verifications.len(),
        items: result_handle.items.as_ptr(),
        handle: Box::into_raw(result_handle),
    };

    let context = context
        .downcast_ref::<StateBlocksVerifiedContext>()
        .unwrap();
    unsafe {
        (context.ffi_callback)(context.ffi_context, &result_dto);
    }
}

//todo
#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_setup_items(
    handle: *mut StateBlockSignatureVerificationHandle,
    max_count: usize,
    result: *mut StateBlockSignatureVerificationValueDto,
) -> usize {
    let verification = unsafe { &(*handle).verification };
    let result = unsafe { std::slice::from_raw_parts_mut(result, max_count) };
    let items = verification.setup_items(max_count);
    for i in 0..items.len() {
        result[i] = (&items[i]).into();
    }
    items.len()
}

// state_blocks
//----------------------------
#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_blocks_empty(
    handle: *const StateBlockSignatureVerificationHandle,
) -> bool {
    let verification = unsafe { &(*handle).verification };
    verification.state_blocks.lock().unwrap().is_empty()
}

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_blocks_push(
    handle: *mut StateBlockSignatureVerificationHandle,
    block: *const StateBlockSignatureVerificationValueDto,
) {
    let verification = unsafe { &mut (*handle).verification };
    let block = unsafe { &*block };
    let block = StateBlockSignatureVerificationValue {
        block: unsafe { &*block.block }.block.clone(),
        account: Account::from_bytes(block.account),
        verification: FromPrimitive::from_u8(block.verification).unwrap(),
    };
    verification.state_blocks.lock().unwrap().push_back(block);
}

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_size(
    handle: *const StateBlockSignatureVerificationHandle,
) -> usize {
    let verification = unsafe { &(*handle).verification };
    verification.state_blocks.lock().unwrap().len()
}

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_blocks_drain(
    handle: *mut StateBlockSignatureVerificationHandle,
    count: usize,
    result: *mut StateBlockSignatureVerificationValueDto,
) {
    let verification = unsafe { &mut (*handle).verification };
    let result = unsafe { std::slice::from_raw_parts_mut(result, count) };
    let mut blocks = verification.state_blocks.lock().unwrap();
    assert_eq!(count, blocks.len());
    for i in 0..count {
        result[i] = (&blocks[i]).into();
    }
    blocks.clear();
}

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_blocks_pop(
    handle: *mut StateBlockSignatureVerificationHandle,
    result: *mut StateBlockSignatureVerificationValueDto,
) {
    let verification = unsafe { &mut (*handle).verification };
    let result = unsafe { &mut *result };
    let mut blocks = verification.state_blocks.lock().unwrap();
    let front = blocks.pop_front().unwrap();
    result.block = Box::into_raw(Box::new(BlockHandle {
        block: front.block.clone(),
    }));
    result.account = front.account.to_bytes();
    result.verification = front.verification as u8;
}
//----------------------------

impl From<&StateBlockSignatureVerificationValue> for StateBlockSignatureVerificationValueDto {
    fn from(value: &StateBlockSignatureVerificationValue) -> Self {
        StateBlockSignatureVerificationValueDto {
            block: Box::into_raw(Box::new(BlockHandle {
                block: value.block.clone(),
            })),
            account: value.account.to_bytes(),
            verification: value.verification as u8,
        }
    }
}
