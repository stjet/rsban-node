use crate::{voting::LocalVoteHistory, BlockHash, Root};

use super::vote::VoteHandle;

pub struct LocalVoteHistoryHandle {
    history: LocalVoteHistory,
}

#[no_mangle]
pub extern "C" fn rsn_local_vote_history_create(max_cache: usize) -> *mut LocalVoteHistoryHandle {
    Box::into_raw(Box::new(LocalVoteHistoryHandle {
        history: LocalVoteHistory::new(max_cache),
    }))
}

#[no_mangle]
pub extern "C" fn rsn_local_vote_history_destroy(handle: *mut LocalVoteHistoryHandle) {
    let uniquer = unsafe { Box::from_raw(handle) };
    drop(uniquer);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_add(
    handle: *mut LocalVoteHistoryHandle,
    root: *const u8,
    hash: *const u8,
    vote: *const VoteHandle,
) {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(std::slice::from_raw_parts(root, 32));
    let root = Root::from_bytes(bytes);

    bytes.copy_from_slice(std::slice::from_raw_parts(hash, 32));
    let hash = BlockHash::from_bytes(bytes);

    let vote = (*vote).vote.clone();

    (*handle).history.add(&root, &hash, &vote);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_erase(
    handle: *mut LocalVoteHistoryHandle,
    root: *const u8,
) {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(std::slice::from_raw_parts(root, 32));
    let root = Root::from_bytes(bytes);
    (*handle).history.erase(&root);
}

#[repr(C)]
pub struct LocalVotesResult {
    pub count: usize,
    pub votes: *const *mut VoteHandle,
    pub handle: *mut LocalVotesResultHandle,
}

pub struct LocalVotesResultHandle(Vec<*mut VoteHandle>);

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_votes(
    handle: *mut LocalVoteHistoryHandle,
    root: *const u8,
    hash: *const u8,
    is_final: bool,
    result: *mut LocalVotesResult,
) {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(std::slice::from_raw_parts(root, 32));
    let root = Root::from_bytes(bytes);

    bytes.copy_from_slice(std::slice::from_raw_parts(hash, 32));
    let hash = BlockHash::from_bytes(bytes);

    let mut votes = (*handle).history.votes(&root, &hash, is_final);
    let mut votes = Box::new(LocalVotesResultHandle(
        votes
            .drain(..)
            .map(|vote| Box::into_raw(Box::new(VoteHandle { vote })))
            .collect::<Vec<_>>(),
    ));
    let result = &mut *result;
    result.count = votes.0.len();
    result.votes = votes.0.as_mut_ptr();
    result.handle = Box::into_raw(votes);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_votes_destroy(handle: *mut LocalVotesResultHandle) {
    let votes = Box::from_raw(handle);
    for x in votes.0 {
        drop(Box::from_raw(x))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_exists(
    handle: *mut LocalVoteHistoryHandle,
    root: *const u8,
) -> bool {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(std::slice::from_raw_parts(root, 32));
    let root = Root::from_bytes(bytes);
    (*handle).history.exists(&root)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_size(handle: *mut LocalVoteHistoryHandle) -> usize {
    (*handle).history.size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_container_info(
    handle: *mut LocalVoteHistoryHandle,
    size: *mut usize,
    count: *mut usize,
) {
    let (s, c) = (*handle).history.container_info();
    *size = s;
    *count = c;
}
