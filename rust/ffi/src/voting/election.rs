use rsnano_core::{utils::system_time_as_nanoseconds, Account, BlockEnum, BlockHash};
use rsnano_node::voting::{Election, ElectionData, VoteInfo};
use std::{
    ops::Deref,
    sync::{Arc, MutexGuard},
    time::{Duration, SystemTime},
};

use crate::{
    copy_account_bytes, copy_hash_bytes, copy_root_bytes,
    core::{copy_block_array_dto, BlockArrayDto, BlockHandle},
};

use super::election_status::ElectionStatusHandle;

pub struct ElectionHandle(Arc<Election>);

impl Deref for ElectionHandle {
    type Target = Arc<Election>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_election_create(block: &BlockHandle) -> *mut ElectionHandle {
    Box::into_raw(Box::new(ElectionHandle(Arc::new(Election::new(
        Arc::clone(block),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_destroy(handle: *mut ElectionHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_election_lock(handle: &ElectionHandle) -> *mut ElectionLockHandle {
    let guard = handle.mutex.lock().unwrap();
    let guard = unsafe {
        std::mem::transmute::<MutexGuard<ElectionData>, MutexGuard<'static, ElectionData>>(guard)
    };
    Box::into_raw(Box::new(ElectionLockHandle(Some(guard))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_root(handle: &ElectionHandle, result: *mut u8) {
    copy_root_bytes(handle.root, result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_qualified_root(
    handle: &ElectionHandle,
    root: *mut u8,
    previous: *mut u8,
) {
    copy_root_bytes(handle.qualified_root.root, root);
    copy_hash_bytes(handle.qualified_root.previous, previous);
}

pub struct ElectionLockHandle(Option<MutexGuard<'static, ElectionData>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_destroy(handle: *mut ElectionLockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_status(
    handle: &ElectionLockHandle,
) -> *mut ElectionStatusHandle {
    Box::into_raw(Box::new(ElectionStatusHandle(
        handle.0.as_ref().unwrap().status.clone(),
    )))
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_status_set(
    handle: &mut ElectionLockHandle,
    status: &ElectionStatusHandle,
) {
    let current = handle.0.as_mut().unwrap();
    current.status = status.deref().clone();
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_unlock(handle: &mut ElectionLockHandle) {
    handle.0.take();
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_lock(
    handle: &mut ElectionLockHandle,
    election: &ElectionHandle,
) {
    assert!(handle.0.is_none());
    let guard = election.mutex.lock().unwrap();
    let guard = unsafe {
        std::mem::transmute::<MutexGuard<ElectionData>, MutexGuard<'static, ElectionData>>(guard)
    };
    handle.0 = Some(guard);
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_add_block(
    handle: &mut ElectionLockHandle,
    block: &BlockHandle,
) {
    handle
        .0
        .as_mut()
        .unwrap()
        .last_blocks
        .insert(block.hash(), Arc::clone(block));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_erase_block(
    handle: &mut ElectionLockHandle,
    hash: *const u8,
) {
    handle
        .0
        .as_mut()
        .unwrap()
        .last_blocks
        .remove(&BlockHash::from_ptr(hash));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_blocks_size(handle: &ElectionLockHandle) -> usize {
    handle.0.as_ref().unwrap().last_blocks.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_blocks_find(
    handle: &ElectionLockHandle,
    hash: *const u8,
) -> *mut BlockHandle {
    match handle
        .0
        .as_ref()
        .unwrap()
        .last_blocks
        .get(&BlockHash::from_ptr(hash))
    {
        Some(block) => Box::into_raw(Box::new(BlockHandle(Arc::clone(block)))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_blocks(
    handle: &ElectionLockHandle,
    result: &mut BlockArrayDto,
) {
    let blocks: Vec<Arc<BlockEnum>> = handle
        .0
        .as_ref()
        .unwrap()
        .last_blocks
        .values()
        .cloned()
        .collect();

    copy_block_array_dto(blocks, result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_votes_insert(
    handle: &mut ElectionLockHandle,
    account: *const u8,
    vote: &VoteInfoHandle,
) {
    handle
        .0
        .as_mut()
        .unwrap()
        .last_votes
        .insert(Account::from_ptr(account), vote.0.clone());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_votes_find(
    handle: &ElectionLockHandle,
    account: *const u8,
) -> *mut VoteInfoHandle {
    match handle
        .0
        .as_ref()
        .unwrap()
        .last_votes
        .get(&Account::from_ptr(account))
    {
        Some(info) => VoteInfoHandle::new(info.clone()),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_votes_size(handle: &ElectionLockHandle) -> usize {
    handle.0.as_ref().unwrap().last_votes.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_votes(
    handle: &ElectionLockHandle,
) -> *mut VoteInfoCollectionHandle {
    let votes = handle
        .0
        .as_ref()
        .unwrap()
        .last_votes
        .iter()
        .map(|(a, i)| (*a, i.clone()))
        .collect::<Vec<_>>();

    Box::into_raw(Box::new(VoteInfoCollectionHandle(votes)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_votes_erase(
    handle: &mut ElectionLockHandle,
    account: *const u8,
) {
    handle
        .0
        .as_mut()
        .unwrap()
        .last_votes
        .remove(&Account::from_ptr(account));
}

pub struct VoteInfoCollectionHandle(Vec<(Account, VoteInfo)>);

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_info_collection_destroy(handle: *mut VoteInfoCollectionHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_vote_info_collection_len(handle: &VoteInfoCollectionHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_info_collection_get(
    handle: &VoteInfoCollectionHandle,
    index: usize,
    account: *mut u8,
) -> *mut VoteInfoHandle {
    let (acc, vote) = &handle.0[index];
    copy_account_bytes(*acc, account);
    return VoteInfoHandle::new(vote.clone());
}

pub struct VoteInfoHandle(VoteInfo);

impl VoteInfoHandle {
    pub fn new(info: VoteInfo) -> *mut VoteInfoHandle {
        Box::into_raw(Box::new(VoteInfoHandle(info)))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_info_create1() -> *mut VoteInfoHandle {
    VoteInfoHandle::new(Default::default())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_info_create2(
    timestamp: u64,
    hash: *const u8,
) -> *mut VoteInfoHandle {
    VoteInfoHandle::new(VoteInfo {
        time: SystemTime::now(),
        timestamp,
        hash: BlockHash::from_ptr(hash),
    })
}

#[no_mangle]
pub extern "C" fn rsn_vote_info_clone(handle: &VoteInfoHandle) -> *mut VoteInfoHandle {
    VoteInfoHandle::new(handle.0.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_info_destroy(handle: *mut VoteInfoHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_vote_info_time_ns(handle: &VoteInfoHandle) -> u64 {
    system_time_as_nanoseconds(handle.0.time)
}

#[no_mangle]
pub extern "C" fn rsn_vote_info_timestamp(handle: &VoteInfoHandle) -> u64 {
    handle.0.timestamp
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_info_hash(handle: &VoteInfoHandle, hash: *mut u8) {
    copy_hash_bytes(handle.0.hash, hash);
}

#[no_mangle]
pub extern "C" fn rsn_vote_info_with_relative_time(
    handle: &VoteInfoHandle,
    seconds: i64,
) -> *mut VoteInfoHandle {
    let delta = Duration::from_secs(seconds.abs() as u64);
    VoteInfoHandle::new(VoteInfo {
        time: if seconds < 0 {
            SystemTime::now() - delta
        } else {
            SystemTime::now() + delta
        },
        timestamp: handle.0.timestamp,
        hash: handle.0.hash,
    })
}
