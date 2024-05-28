use super::VoteHandle;
use crate::transport::ChannelHandle;
use rsnano_node::consensus::{VoteProcessor, VoteProcessorConfig, VoteProcessorExt};
use std::{
    ffi::c_void,
    ops::Deref,
    sync::{atomic::Ordering, Arc},
};

pub struct VoteProcessorHandle(pub Arc<VoteProcessor>);

impl Deref for VoteProcessorHandle {
    type Target = Arc<VoteProcessor>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type VoteProcessorVoteProcessedCallback =
    unsafe extern "C" fn(*mut c_void, *mut VoteHandle, *mut ChannelHandle, u8);

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_processor_destroy(handle: *mut VoteProcessorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_vote_processor_start(handle: &VoteProcessorHandle) {
    handle.0.start();
}

#[no_mangle]
pub extern "C" fn rsn_vote_processor_stop(handle: &VoteProcessorHandle) {
    handle.0.stop();
}

#[no_mangle]
pub extern "C" fn rsn_vote_processor_vote_blocking(
    handle: &VoteProcessorHandle,
    vote: &VoteHandle,
    channel: &ChannelHandle,
) -> u8 {
    handle.0.vote_blocking(vote, &Some(channel.deref().clone())) as u8
}

#[no_mangle]
pub extern "C" fn rsn_vote_processor_total_processed(handle: &VoteProcessorHandle) -> u64 {
    handle.0.total_processed.load(Ordering::SeqCst)
}

#[repr(C)]
pub struct VoteProcessorConfigDto {
    pub max_pr_queue: usize,
    pub max_non_pr_queue: usize,
    pub pr_priority: usize,
}

impl From<&VoteProcessorConfigDto> for VoteProcessorConfig {
    fn from(value: &VoteProcessorConfigDto) -> Self {
        Self {
            max_pr_queue: value.max_pr_queue,
            max_non_pr_queue: value.max_non_pr_queue,
            pr_priority: value.pr_priority,
        }
    }
}

impl From<&VoteProcessorConfig> for VoteProcessorConfigDto {
    fn from(value: &VoteProcessorConfig) -> Self {
        Self {
            max_pr_queue: value.max_pr_queue,
            max_non_pr_queue: value.max_non_pr_queue,
            pr_priority: value.pr_priority,
        }
    }
}
