use std::{
    ffi::c_void,
    sync::{Arc, RwLock},
};

use crate::{Account, BlockHash, Signature, Vote};

use super::{FfiPropertyTreeWriter, StringDto};

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
    hashes: *const [u8; 32],
    hash_count: usize,
) -> *mut VoteHandle {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(unsafe { std::slice::from_raw_parts(account, 32) });
    let account = Account::from_bytes(bytes);

    let hashes = unsafe { std::slice::from_raw_parts(hashes, hash_count) };
    let hashes = hashes.iter().map(|&h| BlockHash::from_bytes(h)).collect();

    Box::into_raw(Box::new(VoteHandle {
        vote: Arc::new(RwLock::new(Vote::new(account, timestamp, duration, hashes))),
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

#[repr(C)]
pub struct VoteHashesDto {
    pub handle: *mut VoteHashesHandle,
    pub count: usize,
    pub hashes: *const [u8; 32],
}

pub struct VoteHashesHandle(Vec<[u8; 32]>);

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_hashes(handle: *const VoteHandle) -> VoteHashesDto {
    let hashes: Vec<_> = (*handle)
        .vote
        .read()
        .unwrap()
        .hashes
        .iter()
        .map(|i| *i.as_bytes())
        .collect();

    let hashes_ptr = hashes.as_ptr();
    let count = hashes.len();
    let handle = Box::into_raw(Box::new(VoteHashesHandle(hashes)));
    VoteHashesDto {
        handle,
        count,
        hashes: hashes_ptr,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_timestamp(handle: *const VoteHandle) -> u64 {
    (*handle).vote.read().unwrap().timestamp()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_duration_bits(handle: *const VoteHandle) -> u8 {
    (*handle).vote.read().unwrap().duration_bits()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_duration_ms(handle: *const VoteHandle) -> u64 {
    (*handle).vote.read().unwrap().duration().as_millis() as u64
}

#[no_mangle]
pub extern "C" fn rsn_vote_hashes_destroy(hashes: *mut VoteHashesHandle) {
    drop(unsafe { Box::from_raw(hashes) });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_hashes_set(
    handle: *mut VoteHandle,
    hashes: *const [u8; 32],
    count: usize,
) {
    let hashes = std::slice::from_raw_parts(hashes, count)
        .iter()
        .map(|&h| BlockHash::from_bytes(h))
        .collect();

    let mut lk = (*handle).vote.write().unwrap();
    lk.hashes = hashes;
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

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_hashes_string(handle: *const VoteHandle) -> StringDto {
    (*handle).vote.read().unwrap().vote_hashes_string().into()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_serialize_json(handle: *const VoteHandle, ptree: *mut c_void) {
    let mut writer = FfiPropertyTreeWriter::new_borrowed(ptree);
    (*handle)
        .vote
        .read()
        .unwrap()
        .serialize_json(&mut writer)
        .unwrap();
}
