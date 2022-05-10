use std::sync::{Arc, RwLock};

use crate::vote::Vote;

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
pub extern "C" fn rsn_vote_create2(timestamp: u64, duration: u8) -> *mut VoteHandle {
    Box::into_raw(Box::new(VoteHandle {
        vote: Arc::new(RwLock::new(Vote::new(timestamp, duration))),
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
