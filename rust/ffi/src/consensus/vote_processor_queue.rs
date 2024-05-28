use super::{rep_tiers::RepTiersHandle, VoteHandle, VoteProcessorConfigDto};
use crate::{
    ledger::datastore::LedgerHandle, representatives::OnlineRepsHandle, transport::ChannelHandle,
    StatHandle,
};
use rsnano_node::consensus::VoteProcessorQueue;
use std::{ops::Deref, sync::Arc};

pub struct VoteProcessorQueueHandle(pub Arc<VoteProcessorQueue>);

impl Deref for VoteProcessorQueueHandle {
    type Target = Arc<VoteProcessorQueue>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_processor_queue_create(
    config: &VoteProcessorConfigDto,
    stats: &StatHandle,
    online_reps: &OnlineRepsHandle,
    ledger: &LedgerHandle,
    rep_tiers: &RepTiersHandle,
) -> *mut VoteProcessorQueueHandle {
    Box::into_raw(Box::new(VoteProcessorQueueHandle(Arc::new(
        VoteProcessorQueue::new(
            config.into(),
            Arc::clone(stats),
            Arc::clone(online_reps),
            Arc::clone(ledger),
            Arc::clone(rep_tiers),
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_processor_queue_destroy(handle: *mut VoteProcessorQueueHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_vote_processor_queue_len(handle: &VoteProcessorQueueHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub extern "C" fn rsn_vote_processor_queue_is_empty(handle: &VoteProcessorQueueHandle) -> bool {
    handle.0.is_empty()
}

#[no_mangle]
pub extern "C" fn rsn_vote_processor_queue_vote(
    handle: &VoteProcessorQueueHandle,
    vote: &VoteHandle,
    channel: &ChannelHandle,
) -> bool {
    handle.0.vote(vote, channel)
}

#[no_mangle]
pub extern "C" fn rsn_vote_processor_queue_stop(handle: &VoteProcessorQueueHandle) {
    handle.0.stop();
}
