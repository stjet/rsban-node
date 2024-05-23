use super::ChannelHandle;
use crate::messages::MessageHandle;
use rsnano_node::transport::LiveMessageProcessor;
use std::sync::Arc;

pub struct LiveMessageProcessorHandle(pub Arc<LiveMessageProcessor>);

#[no_mangle]
pub unsafe extern "C" fn rsn_live_message_processor_destroy(
    handle: *mut LiveMessageProcessorHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_live_message_processor_process(
    handle: &LiveMessageProcessorHandle,
    message: &MessageHandle,
    channel: &ChannelHandle,
) {
    handle.0.process(message.message.clone(), channel)
}
