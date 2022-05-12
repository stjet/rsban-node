use std::sync::Arc;

use super::vote::VoteHandle;
use crate::VoteUniquer;

pub struct VoteUniquerHandle {
    uniquer: VoteUniquer,
}

#[no_mangle]
pub extern "C" fn rsn_vote_uniquer_create() -> *mut VoteUniquerHandle {
    Box::into_raw(Box::new(VoteUniquerHandle {
        uniquer: VoteUniquer::new(),
    }))
}

#[no_mangle]
pub extern "C" fn rsn_vote_uniquer_destroy(handle: *mut VoteUniquerHandle) {
    let uniquer = unsafe { Box::from_raw(handle) };
    drop(uniquer);
}

#[no_mangle]
pub extern "C" fn rsn_vote_uniquer_size(handle: *const VoteUniquerHandle) -> usize {
    unsafe { &*handle }.uniquer.size()
}

#[no_mangle]
pub extern "C" fn rsn_vote_uniquer_unique(
    handle: *mut VoteUniquerHandle,
    vote: *mut VoteHandle,
) -> *mut VoteHandle {
    let original = &unsafe { &*vote }.vote;
    let uniqued = unsafe { &*handle }.uniquer.unique(original);
    if Arc::ptr_eq(&uniqued, original) {
        vote
    } else {
        Box::into_raw(Box::new(VoteHandle { vote: uniqued }))
    }
}
