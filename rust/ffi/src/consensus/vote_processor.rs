use super::{vote_processor_queue::VoteProcessorQueueHandle, ActiveTransactionsHandle, VoteHandle};
use crate::{transport::ChannelHandle, utils::ContextWrapper, StatHandle, VoidPointerCallback};
use rsnano_core::{Vote, VoteCode};
use rsnano_node::{
    consensus::{VoteProcessor, VoteProcessorExt},
    transport::ChannelEnum,
};
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
pub unsafe extern "C" fn rsn_vote_processor_create(
    queue: &VoteProcessorQueueHandle,
    active: &ActiveTransactionsHandle,
    stats: &StatHandle,
    vote_processed: VoteProcessorVoteProcessedCallback,
    callback_context: *mut c_void,
    delete_context: VoidPointerCallback,
) -> *mut VoteProcessorHandle {
    let context_wrapper = ContextWrapper::new(callback_context, delete_context);
    let processed = Box::new(
        move |vote: &Arc<Vote>, channel: &Arc<ChannelEnum>, code: VoteCode| {
            let vote_handle = VoteHandle::new(Arc::clone(vote));
            let channel_handle = ChannelHandle::new(Arc::clone(channel));
            vote_processed(
                context_wrapper.get_context(),
                vote_handle,
                channel_handle,
                code as u8,
            );
        },
    );
    Box::into_raw(Box::new(VoteProcessorHandle(Arc::new(VoteProcessor::new(
        Arc::clone(queue),
        Arc::clone(active),
        Arc::clone(stats),
        processed,
    )))))
}

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
    validated: bool,
) -> u8 {
    handle.0.vote_blocking(vote, channel, validated) as u8
}

#[no_mangle]
pub extern "C" fn rsn_vote_processor_total_processed(handle: &VoteProcessorHandle) -> u64 {
    handle.0.total_processed.load(Ordering::SeqCst)
}
