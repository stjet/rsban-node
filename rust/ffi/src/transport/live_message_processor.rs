use rsnano_node::transport::LiveMessageProcessor;
use std::sync::Arc;

use crate::{
    block_processing::BlockProcessorHandle,
    bootstrap::{BootstrapAscendingHandle, BootstrapServerHandle},
    consensus::{RequestAggregatorHandle, VoteProcessorQueueHandle},
    messages::MessageHandle,
    telemetry::TelemetryHandle,
    wallets::LmdbWalletsHandle,
    NodeConfigDto, NodeFlagsHandle, StatHandle,
};

use super::{ChannelHandle, TcpChannelsHandle};

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

#[no_mangle]
pub unsafe extern "C" fn rsn_live_message_processor_bind(
    handle: &LiveMessageProcessorHandle,
    channels: &TcpChannelsHandle,
) {
    let processor = Arc::downgrade(&handle.0);
    channels.set_sink(Box::new(move |msg, channel| {
        if let Some(processor) = processor.upgrade() {
            processor.process(msg.message, &channel);
        }
    }));
}
