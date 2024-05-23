use super::vote::VoteHandle;
use rsnano_core::{BlockHash, Root};
use rsnano_node::consensus::LocalVoteHistory;
use std::{ops::Deref, sync::Arc};

pub struct LocalVoteHistoryHandle(pub Arc<LocalVoteHistory>);

impl Deref for LocalVoteHistoryHandle {
    type Target = Arc<LocalVoteHistory>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_local_vote_history_create(max_cache: usize) -> *mut LocalVoteHistoryHandle {
    Box::into_raw(Box::new(LocalVoteHistoryHandle(Arc::new(
        LocalVoteHistory::new(max_cache),
    ))))
}

#[no_mangle]
pub extern "C" fn rsn_local_vote_history_destroy(handle: *mut LocalVoteHistoryHandle) {
    let uniquer = unsafe { Box::from_raw(handle) };
    drop(uniquer);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_add(
    handle: &LocalVoteHistoryHandle,
    root: *const u8,
    hash: *const u8,
    vote: *const VoteHandle,
) {
    let root = Root::from_ptr(root);
    let hash = BlockHash::from_ptr(hash);
    let vote = (*vote).clone();
    handle.add(&root, &hash, &vote);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_erase(
    handle: &LocalVoteHistoryHandle,
    root: *const u8,
) {
    let root = Root::from_ptr(root);
    handle.erase(&root);
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
    handle: &LocalVoteHistoryHandle,
    root: *const u8,
    hash: *const u8,
    is_final: bool,
    result: *mut LocalVotesResult,
) {
    let root = Root::from_ptr(root);
    let hash = BlockHash::from_ptr(hash);

    let mut votes = handle.votes(&root, &hash, is_final);
    let mut votes = Box::new(LocalVotesResultHandle(
        votes
            .drain(..)
            .map(|vote| VoteHandle::new(vote))
            .collect::<Vec<_>>(),
    ));
    let result = &mut *result;
    result.count = votes.0.len();
    result.votes = votes.0.as_mut_ptr();
    result.handle = Box::into_raw(votes);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_votes_destroy(handle: *mut LocalVotesResultHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_exists(
    handle: &LocalVoteHistoryHandle,
    root: *const u8,
) -> bool {
    let root = Root::from_ptr(root);
    handle.exists(&root)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_vote_history_size(handle: &LocalVoteHistoryHandle) -> usize {
    handle.size()
}
