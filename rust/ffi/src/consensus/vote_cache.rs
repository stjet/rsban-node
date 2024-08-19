use rsnano_core::{BlockHash, VoteCode};
use rsnano_node::consensus::VoteCacheConfig;
use std::{collections::HashMap, time::Duration};

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
