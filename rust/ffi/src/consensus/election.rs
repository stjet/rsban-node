use num_traits::FromPrimitive;
use rsnano_core::{utils::system_time_as_nanoseconds, Account, Amount, BlockEnum, BlockHash};
use rsnano_node::{
    consensus::{
        Election, ElectionBehavior, ElectionData, ElectionState, ElectionStatusType, VoteInfo,
    },
    stats::DetailType,
};
use std::{
    ffi::c_void,
    ops::Deref,
    sync::{atomic::Ordering, Arc, MutexGuard},
    time::{Duration, SystemTime},
};

use crate::{
    core::{copy_block_array_dto, BlockArrayDto, BlockHandle},
    utils::ContextWrapper,
    VoidPointerCallback,
};

use super::election_status::ElectionStatusHandle;

pub struct ElectionHandle(pub Arc<Election>);

impl Deref for ElectionHandle {
    type Target = Arc<Election>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type ConfirmationAction = unsafe extern "C" fn(*mut c_void, *mut BlockHandle);
pub type LiveVoteAction = unsafe extern "C" fn(*mut c_void, *const u8);

#[no_mangle]
pub unsafe extern "C" fn rsn_election_create(
    block: &BlockHandle,
    behavior: u8,
    confirmation_action: ConfirmationAction,
    confirmation_action_context: *mut c_void,
    confirmation_action_context_delete: VoidPointerCallback,
    live_vote_action: LiveVoteAction,
    live_vote_action_context: *mut c_void,
    live_vote_action_context_delete: VoidPointerCallback,
) -> *mut ElectionHandle {
    let confirmation_context = ContextWrapper::new(
        confirmation_action_context,
        confirmation_action_context_delete,
    );

    let live_vote_context =
        ContextWrapper::new(live_vote_action_context, live_vote_action_context_delete);

    Box::into_raw(Box::new(ElectionHandle(Arc::new(Election::new(
        Arc::clone(block),
        ElectionBehavior::from_u8(behavior).unwrap(),
        Box::new(move |block| {
            confirmation_action(
                confirmation_context.get_context(),
                Box::into_raw(Box::new(BlockHandle(block))),
            );
        }),
        Box::new(move |account| {
            live_vote_action(live_vote_context.get_context(), account.as_bytes().as_ptr())
        }),
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
    handle.root.copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_qualified_root(
    handle: &ElectionHandle,
    root: *mut u8,
    previous: *mut u8,
) {
    handle.qualified_root.root.copy_bytes(root);
    handle.qualified_root.previous.copy_bytes(previous);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_is_quorum(handle: &ElectionHandle) -> bool {
    handle.0.is_quorum.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_is_quorum_exchange(
    handle: &ElectionHandle,
    value: bool,
) -> bool {
    handle.0.is_quorum.swap(value, Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_last_block_elapsed_ms(handle: &ElectionHandle) -> u64 {
    handle.0.last_block_elapsed().as_millis() as u64
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_set_last_block(handle: &ElectionHandle) {
    handle.0.set_last_block();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_confirmation_request_count(handle: &ElectionHandle) -> u32 {
    handle.0.confirmation_request_count.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_confirmation_request_count_inc(handle: &ElectionHandle) {
    handle
        .0
        .confirmation_request_count
        .fetch_add(1, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn rsn_election_behavior(handle: &ElectionHandle) -> u8 {
    handle.0.behavior as u8
}

#[no_mangle]
pub extern "C" fn rsn_election_elapsed_ms(handle: &ElectionHandle) -> u64 {
    handle.0.election_start.elapsed().as_millis() as u64
}

#[no_mangle]
pub extern "C" fn rsn_election_last_req_set(handle: &ElectionHandle) {
    handle.0.set_last_req();
}

#[no_mangle]
pub extern "C" fn rsn_election_last_req_elapsed_ms(handle: &ElectionHandle) -> u64 {
    handle.0.last_req_elapsed().as_millis() as u64
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_last_vote_set(handle: &mut ElectionLockHandle) {
    handle.0.as_mut().unwrap().set_last_vote();
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_last_vote_elapsed_ms(handle: &ElectionLockHandle) -> u64 {
    handle.0.as_ref().unwrap().last_vote_elapsed().as_millis() as u64
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_live_vote_action(
    handle: &ElectionHandle,
    account: *const u8,
) {
    let account = Account::from_ptr(account);
    (handle.0.live_vote_action)(account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_confirmation_action(
    handle: &ElectionHandle,
    block: &BlockHandle,
) {
    let block = Arc::clone(block);
    (handle.0.confirmation_action)(block);
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
pub extern "C" fn rsn_election_lock_state_change(
    handle: &mut ElectionLockHandle,
    expected_state: u8,
    desired_state: u8,
) -> bool {
    let expected = ElectionState::from_u8(expected_state).unwrap();
    let desired = ElectionState::from_u8(desired_state).unwrap();
    handle
        .0
        .as_mut()
        .unwrap()
        .state_change(expected, desired)
        .is_err()
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_state_start_elapsed_ms(handle: &ElectionLockHandle) -> u64 {
    handle.0.as_ref().unwrap().state_start.elapsed().as_millis() as u64
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_update_status_to_confirmed(
    lock_handle: &mut ElectionLockHandle,
    election_handle: &ElectionHandle,
    status_type: u8,
) {
    let status_type = ElectionStatusType::from_u8(status_type).unwrap();
    lock_handle
        .0
        .as_mut()
        .unwrap()
        .update_status_to_confirmed(status_type, &election_handle.0);
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
pub extern "C" fn rsn_election_lock_state(handle: &ElectionLockHandle) -> u8 {
    handle.0.as_ref().unwrap().state as u8
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_state_set(handle: &mut ElectionLockHandle, state: u8) {
    handle.0.as_mut().unwrap().state = FromPrimitive::from_u8(state).unwrap()
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
pub unsafe extern "C" fn rsn_election_lock_final_weight(
    handle: &ElectionLockHandle,
    weight: *mut u8,
) {
    handle.0.as_ref().unwrap().final_weight.copy_bytes(weight);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_final_weight_set(
    handle: &mut ElectionLockHandle,
    weight: *const u8,
) {
    handle.0.as_mut().unwrap().final_weight = Amount::from_ptr(weight);
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
pub unsafe extern "C" fn rsn_election_lock_last_tally_clear(handle: &mut ElectionLockHandle) {
    handle.0.as_mut().unwrap().last_tally.clear();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_last_tally_add(
    handle: &mut ElectionLockHandle,
    hash: *const u8,
    amount: *const u8,
) {
    handle
        .0
        .as_mut()
        .unwrap()
        .last_tally
        .insert(BlockHash::from_ptr(hash), Amount::from_ptr(amount));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_last_tally(
    handle: &ElectionLockHandle,
) -> *mut TallyHandle {
    let tally_vec = handle
        .0
        .as_ref()
        .unwrap()
        .last_tally
        .iter()
        .map(|(k, v)| (*k, *v))
        .collect();
    Box::into_raw(Box::new(TallyHandle(tally_vec)))
}

pub struct TallyHandle(Vec<(BlockHash, Amount)>);

#[no_mangle]
pub unsafe extern "C" fn rsn_tally_destroy(handle: *mut TallyHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tally_len(handle: &TallyHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tally_get(
    handle: &TallyHandle,
    index: usize,
    hash: *mut u8,
    tally: *mut u8,
) {
    let (hash_value, tally_value) = &handle.0[index];
    hash_value.copy_bytes(hash);
    tally_value.copy_bytes(tally);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_(handle: &mut ElectionLockHandle, hash: *const u8) {
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
    acc.copy_bytes(account);
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
    handle.0.hash.copy_bytes(hash);
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

#[no_mangle]
pub extern "C" fn rsn_election_behaviour_into_stat_detail(behaviour: u8) -> u8 {
    let detail: DetailType = ElectionBehavior::from_u8(behaviour).unwrap().into();
    detail as u8
}
