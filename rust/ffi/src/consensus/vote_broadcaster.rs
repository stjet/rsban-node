use super::VoteHandle;
use crate::representatives::RepresentativeRegisterHandle;
use rsnano_node::consensus::VoteBroadcaster;
use std::sync::Arc;
pub struct VoteBroadcasterHandle(VoteBroadcaster);

#[no_mangle]
pub extern "C" fn rsn_vote_broadcaster_create(
    rep_register: &RepresentativeRegisterHandle,
) -> *mut VoteBroadcasterHandle {
    Box::into_raw(Box::new(VoteBroadcasterHandle(VoteBroadcaster {
        representative_register: Arc::clone(rep_register),
    })))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_broadcaster_destroy(handle: *mut VoteBroadcasterHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_vote_broadcaster_broadcast(
    handle: &VoteBroadcasterHandle,
    vote: &VoteHandle,
) {
    handle.0.broadcast(Arc::clone(vote));
}
