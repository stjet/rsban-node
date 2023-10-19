use crate::{copy_account_bytes, utils::ContainerInfoComponentHandle, voting::VoteHandle};
use rsnano_core::{Amount, BlockHash};
use rsnano_node::vote_cache::{TopEntry, VoteCache, VoterEntry};
use std::{
    ffi::{c_char, CStr},
    sync::{Arc, Mutex},
};

pub struct VoteCacheHandle(Arc<Mutex<VoteCache>>);

#[no_mangle]
pub extern "C" fn rsn_vote_cache_create(max_size: usize) -> *mut VoteCacheHandle {
    Box::into_raw(Box::new(VoteCacheHandle(Arc::new(Mutex::new(
        VoteCache::new(max_size),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_destroy(handle: *mut VoteCacheHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_vote(
    handle: *mut VoteCacheHandle,
    hash: *const u8,
    vote: &VoteHandle,
    rep_weight: *const u8,
) {
    let hash = BlockHash::from_ptr(hash);
    let rep_weight = Amount::from_ptr(rep_weight);
    (*handle).0.lock().unwrap().vote(&hash, &vote, rep_weight);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_cache_empty(handle: *const VoteCacheHandle) -> bool {
    (*handle).0.lock().unwrap().cache_empty()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_queue_empty(handle: *const VoteCacheHandle) -> bool {
    (*handle).0.lock().unwrap().queue_empty()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_cache_size(handle: *const VoteCacheHandle) -> usize {
    (*handle).0.lock().unwrap().cache_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_queue_size(handle: *const VoteCacheHandle) -> usize {
    (*handle).0.lock().unwrap().queue_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_find(
    handle: *mut VoteCacheHandle,
    hash: *const u8,
    result: *mut VoteCacheEntryDto,
) -> bool {
    let hash = BlockHash::from_ptr(hash);
    let guard = (*handle).0.lock().unwrap();
    let entry = guard.find(&hash);
    fill_entry_dto(entry, result)
}

unsafe fn fill_entry_dto(
    entry: Option<&rsnano_node::vote_cache::CacheEntry>,
    result: *mut VoteCacheEntryDto,
) -> bool {
    match entry {
        Some(entry) => {
            (*result).hash.copy_from_slice(entry.hash.as_bytes());
            (*result).tally.copy_from_slice(&entry.tally.to_be_bytes());
            (*result)
                .final_tally
                .copy_from_slice(&entry.final_tally.to_be_bytes());
            (*result).voters_count = entry.voters.len();
            (*result).voters = Box::into_raw(Box::new(VoterListDto(entry.voters.clone())));
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_erase(
    handle: *mut VoteCacheHandle,
    hash: *const u8,
) -> bool {
    let hash = BlockHash::from_ptr(hash);
    let mut guard = (*handle).0.lock().unwrap();
    guard.erase(&hash)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_pop(
    handle: *mut VoteCacheHandle,
    min_tally: *const u8,
    result: *mut VoteCacheEntryDto,
) -> bool {
    let min_tally = Amount::from_ptr(min_tally);
    let mut guard = (*handle).0.lock().unwrap();
    let entry = guard.pop_min_tally(min_tally);
    fill_entry_dto(entry.as_ref(), result)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_peek(
    handle: *mut VoteCacheHandle,
    min_tally: *const u8,
    result: *mut VoteCacheEntryDto,
) -> bool {
    let min_tally = Amount::from_ptr(min_tally);
    let guard = (*handle).0.lock().unwrap();
    let entry = guard.peek_min_tally(min_tally);
    fill_entry_dto(entry, result)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_trigger(handle: *mut VoteCacheHandle, hash: *const u8) {
    let hash = BlockHash::from_ptr(hash);
    (*handle).0.lock().unwrap().trigger(&hash);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_entry_destroy(entry: *mut VoteCacheEntryDto) {
    drop(Box::from_raw((*entry).voters));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_entry_get_voter(
    entry: *const VoteCacheEntryDto,
    index: usize,
    account: *mut u8,
    timestamp: *mut u64,
) {
    let voter = (*(*entry).voters).0.get(index).unwrap();
    copy_account_bytes(voter.representative, account);
    *timestamp = voter.timestamp;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_collect_container_info(
    handle: *const VoteCacheHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = (*handle)
        .0
        .lock()
        .unwrap()
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}

#[repr(C)]
pub struct VoteCacheEntryDto {
    pub hash: [u8; 32],
    pub tally: [u8; 16],
    pub final_tally: [u8; 16],
    pub voters: *mut VoterListDto,
    pub voters_count: usize,
}

pub struct VoterListDto(Vec<VoterEntry>);

#[repr(C)]
pub struct TopEntryDto {
    pub hash: [u8; 32],
    pub tally: [u8; 16],
    pub final_tally: [u8; 16],
}

pub struct TopEntryVecHandle(Vec<TopEntry>);

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_top(
    handle: &VoteCacheHandle,
    min_tally: *const u8,
) -> *mut TopEntryVecHandle {
    let result = handle.0.lock().unwrap().top(Amount::from_ptr(min_tally));
    Box::into_raw(Box::new(TopEntryVecHandle(result)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_top_entry_vec_destroy(handle: *mut TopEntryVecHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_top_entry_vec_len(handle: &TopEntryVecHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub extern "C" fn rsn_top_entry_vec_get(
    handle: &TopEntryVecHandle,
    index: usize,
    result: &mut TopEntryDto,
) {
    let entry = handle.0.get(index).unwrap();
    result.hash = *entry.hash.as_bytes();
    result.tally = entry.tally.to_be_bytes();
    result.final_tally = entry.final_tally.to_be_bytes();
}
