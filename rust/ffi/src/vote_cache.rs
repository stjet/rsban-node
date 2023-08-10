use crate::{utils::ContainerInfoComponentHandle, voting::VoteHandle, copy_account_bytes};
use rsnano_core::{Account, Amount, BlockHash};
use rsnano_node::vote_cache::VoteCache;
use std::{
    ffi::{c_char, c_void, CStr},
    sync::{Arc, Mutex},
};

pub struct VoteCacheHandle(Arc<Mutex<VoteCache>>);
pub type DeleteRepWeightQueryCallback = unsafe extern "C" fn(*mut c_void);
pub type ExecuteRepWeightQueryCallback = unsafe extern "C" fn(*mut c_void, *const u8, *mut u8);

struct FfiRepWeightQueryWrapper {
    handle: *mut c_void,
    execute_callback: ExecuteRepWeightQueryCallback,
    delete_callback: DeleteRepWeightQueryCallback,
}

impl FfiRepWeightQueryWrapper {
    pub fn execute(&self, rep: &Account) -> Amount {
        unsafe {
            let bytes = rep.as_bytes();
            let mut amount = [0u8; 16];
            (self.execute_callback)(self.handle, bytes.as_ptr(), amount.as_mut_ptr());
            Amount::from_be_bytes(amount)
        }
    }
}

impl Drop for FfiRepWeightQueryWrapper {
    fn drop(&mut self) {
        unsafe { (self.delete_callback)(self.handle) }
    }
}

#[no_mangle]
pub extern "C" fn rsn_vote_cache_create(
    max_size: usize,
    rep_weight_query_handle: *mut c_void,
    execute_rep_weight_query: ExecuteRepWeightQueryCallback,
    delete_rep_weight_query: DeleteRepWeightQueryCallback,
) -> *mut VoteCacheHandle {
    let rep_query_wrapper = FfiRepWeightQueryWrapper {
        handle: rep_weight_query_handle,
        execute_callback: execute_rep_weight_query,
        delete_callback: delete_rep_weight_query,
    };
    let rep_query = Box::new(move |rep: &_| rep_query_wrapper.execute(rep));
    Box::into_raw(Box::new(VoteCacheHandle(Arc::new(Mutex::new(
        VoteCache::new(max_size, rep_query),
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
    vote: *const VoteHandle,
) {
    let hash = BlockHash::from_ptr(hash);
    let vote = (*vote).0.read().unwrap();
    (*handle).0.lock().unwrap().vote(&hash, &vote)
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

unsafe fn fill_entry_dto(entry: Option<&rsnano_node::vote_cache::CacheEntry>, result: *mut VoteCacheEntryDto) -> bool {
    match entry {
        Some(entry) => {
            (*result).hash.copy_from_slice(entry.hash.as_bytes());
            (*result).tally.copy_from_slice(&entry.tally.to_be_bytes());
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
    let entry = guard.pop(Some(min_tally));
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
    let entry = guard.peek(Some(min_tally));
    fill_entry_dto(entry, result)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_trigger(
    handle: *mut VoteCacheHandle,
    hash: *const u8,
) {
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
    let (rep, ts) = (*(*entry).voters).0.get(index).unwrap();
    copy_account_bytes(*rep, account);
    *timestamp = *ts;
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
    pub voters: *mut VoterListDto,
    pub voters_count: usize,
}

pub struct VoterListDto(Vec<(Account, u64)>);
