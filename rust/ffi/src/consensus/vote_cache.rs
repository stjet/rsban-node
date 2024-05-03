use crate::{consensus::VoteHandle, utils::ContainerInfoComponentHandle, StatHandle};
use num_traits::FromPrimitive;
use rsnano_core::{Amount, BlockHash, Vote, VoteCode, VoteSource};
use rsnano_node::consensus::{CacheEntry, TopEntry, VoteCache, VoteCacheConfig};
use std::{
    collections::HashMap,
    ffi::{c_char, CStr},
    ops::Deref,
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct VoteCacheHandle(Arc<Mutex<VoteCache>>);

impl Deref for VoteCacheHandle {
    type Target = Arc<Mutex<VoteCache>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_vote_cache_create(
    config: &VoteCacheConfigDto,
    stats: &StatHandle,
) -> *mut VoteCacheHandle {
    let config = VoteCacheConfig::from(config);
    Box::into_raw(Box::new(VoteCacheHandle(Arc::new(Mutex::new(
        VoteCache::new(config, Arc::clone(stats)),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_destroy(handle: *mut VoteCacheHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_cache_empty(handle: *const VoteCacheHandle) -> bool {
    (*handle).0.lock().unwrap().empty()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_cache_size(handle: *const VoteCacheHandle) -> usize {
    (*handle).0.lock().unwrap().size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_find(
    handle: *mut VoteCacheHandle,
    hash: *const u8,
) -> *mut VoteVecHandle {
    let hash = BlockHash::from_ptr(hash);
    let guard = (*handle).0.lock().unwrap();
    VoteVecHandle::new(guard.find(&hash))
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
pub unsafe extern "C" fn rsn_vote_cache_clear(handle: *mut VoteCacheHandle) {
    let mut guard = (*handle).0.lock().unwrap();
    guard.clear()
}

pub struct VoteResultMapHandle(Vec<(BlockHash, VoteCode)>);

impl VoteResultMapHandle {
    pub fn new(map: &HashMap<BlockHash, VoteCode>) -> *mut Self {
        Box::into_raw(Box::new(Self(map.iter().map(|(k, v)| (*k, *v)).collect())))
    }
}

impl From<&VoteResultMapHandle> for HashMap<BlockHash, VoteCode> {
    fn from(value: &VoteResultMapHandle) -> Self {
        value.0.iter().map(|(k, v)| (*k, *v)).collect()
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_result_map_create() -> *mut VoteResultMapHandle {
    VoteResultMapHandle::new(&HashMap::new())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_result_map_destroy(handle: *mut VoteResultMapHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_result_map_len(handle: &VoteResultMapHandle) -> usize {
    handle.0.len()
}
#[no_mangle]

pub unsafe extern "C" fn rsn_vote_result_map_get(
    handle: &VoteResultMapHandle,
    index: usize,
    hash: *mut u8,
) -> u8 {
    let (block_hash, code) = &handle.0[index];
    block_hash.copy_bytes(hash);
    *code as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_result_map_insert(
    handle: &mut VoteResultMapHandle,
    hash: *const u8,
    code: u8,
) {
    handle.0.push((
        BlockHash::from_ptr(hash),
        FromPrimitive::from_u8(code).unwrap(),
    ));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_observe(
    handle: &mut VoteCacheHandle,
    vote: &VoteHandle,
    rep_weight: *const u8,
    vote_source: u8,
    results: &VoteResultMapHandle,
) {
    handle.0.lock().unwrap().observe(
        vote,
        Amount::from_ptr(rep_weight),
        VoteSource::from_u8(vote_source).unwrap(),
        results.into(),
    );
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_vote(
    handle: &mut VoteCacheHandle,
    vote: &VoteHandle,
    weight: *const u8,
) {
    // TODO: pass filter callback too!
    handle
        .0
        .lock()
        .unwrap()
        .insert(vote, Amount::from_ptr(weight));
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

#[repr(C)]
pub struct VoteCacheConfigDto {
    pub max_size: usize,
    pub max_voters: usize,
    pub age_cutoff_s: u64,
}

impl From<&VoteCacheConfig> for VoteCacheConfigDto {
    fn from(value: &VoteCacheConfig) -> Self {
        Self {
            max_size: value.max_size,
            max_voters: value.max_voters,
            age_cutoff_s: value.age_cutoff.as_secs(),
        }
    }
}

impl From<&VoteCacheConfigDto> for VoteCacheConfig {
    fn from(value: &VoteCacheConfigDto) -> Self {
        Self {
            max_size: value.max_size,
            max_voters: value.max_voters,
            age_cutoff: Duration::from_secs(value.age_cutoff_s),
        }
    }
}

pub struct VoteCacheEntryHandle(CacheEntry);

impl Deref for VoteCacheEntryHandle {
    type Target = CacheEntry;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_entry_destroy(handle: *mut VoteCacheEntryHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_vote_cache_entry_size(handle: &VoteCacheEntryHandle) -> usize {
    handle.size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_entry_hash(handle: &VoteCacheEntryHandle, result: *mut u8) {
    handle.hash.copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_entry_tally(
    handle: &VoteCacheEntryHandle,
    result: *mut u8,
) {
    handle.tally().copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_entry_final_tally(
    handle: &VoteCacheEntryHandle,
    result: *mut u8,
) {
    handle.final_tally().copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_entry_votes(
    handle: &VoteCacheEntryHandle,
) -> *mut VoteVecHandle {
    VoteVecHandle::new(handle.votes())
}

pub struct VoteVecHandle(Vec<Arc<Vote>>);

impl VoteVecHandle {
    pub fn new(votes: Vec<Arc<Vote>>) -> *mut Self {
        Box::into_raw(Box::new(VoteVecHandle(votes)))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_vec_destroy(handle: *mut VoteVecHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_vec_len(handle: &VoteVecHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_vec_get(handle: &VoteVecHandle, index: usize) -> *mut VoteHandle {
    VoteHandle::new(Arc::clone(handle.0.get(index).unwrap()))
}
