use std::sync::{Arc, RwLock};

use crate::{vote::Vote, Account, Signature};

pub struct VoteHandle {
    vote: Arc<RwLock<Vote>>,
}

#[no_mangle]
pub extern "C" fn rsn_vote_create() -> *mut VoteHandle {
    Box::into_raw(Box::new(VoteHandle {
        vote: Arc::new(RwLock::new(Vote::null())),
    }))
}

#[no_mangle]
pub extern "C" fn rsn_vote_create2(
    account: *const u8,
    timestamp: u64,
    duration: u8,
) -> *mut VoteHandle {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(unsafe { std::slice::from_raw_parts(account, 32) });
    let account = Account::from_bytes(bytes);
    Box::into_raw(Box::new(VoteHandle {
        vote: Arc::new(RwLock::new(Vote::new(account, timestamp, duration))),
    }))
}

#[no_mangle]
pub extern "C" fn rsn_vote_destroy(handle: *mut VoteHandle) {
    drop(unsafe { Box::from_raw(handle) })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_copy(handle: *const VoteHandle) -> *mut VoteHandle {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let lk = (*handle).vote.read().unwrap();
    Box::into_raw(Box::new(VoteHandle {
        vote: Arc::new(RwLock::new(lk.clone())),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_timestamp_raw(handle: *const VoteHandle) -> u64 {
    (*handle).vote.read().unwrap().timestamp
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_timestamp_raw_set(handle: *mut VoteHandle, timestamp: u64) {
    (*handle).vote.write().unwrap().timestamp = timestamp;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_account(handle: *const VoteHandle, result: *mut u8) {
    let lk = (*handle).vote.read().unwrap();
    let result = std::slice::from_raw_parts_mut(result, 32);
    result.copy_from_slice(lk.voting_account.as_bytes());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_account_set(handle: *mut VoteHandle, account: *const u8) {
    let mut lk = (*handle).vote.write().unwrap();
    let mut bytes = [0; 32];
    bytes.copy_from_slice(std::slice::from_raw_parts(account, 32));
    lk.voting_account = Account::from_bytes(bytes);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_signature(handle: *const VoteHandle, result: *mut u8) {
    let lk = (*handle).vote.read().unwrap();
    let result = std::slice::from_raw_parts_mut(result, 64);
    result.copy_from_slice(lk.signature.as_bytes());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_signature_set(handle: *mut VoteHandle, signature: *const u8) {
    let mut lk = (*handle).vote.write().unwrap();
    let mut bytes = [0; 64];
    bytes.copy_from_slice(std::slice::from_raw_parts(signature, 64));
    lk.signature = Signature::from_bytes(bytes);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_equals(
    first: *const VoteHandle,
    second: *const VoteHandle,
) -> bool {
    if first.is_null() && second.is_null() {
        return true;
    }

    if first.is_null() || second.is_null() {
        return false;
    }

    (*first)
        .vote
        .read()
        .unwrap()
        .eq(&(*second).vote.read().unwrap())
}
