use super::{vote_processor_queue::VoteProcessorQueueHandle, VoteHandle};
use crate::{
    messages::MessageHandle,
    representatives::RepresentativeRegisterHandle,
    transport::{ChannelHandle, EndpointDto, FfiInboundCallback, TcpChannelsHandle},
    utils::{AsyncRuntimeHandle, ContextWrapper},
    NetworkConstantsDto, StatHandle, VoidPointerCallback,
};
use rsnano_core::Account;
use rsnano_messages::DeserializedMessage;
use rsnano_node::{config::NetworkConstants, consensus::VoteBroadcaster};
use std::{ffi::c_void, sync::Arc};
pub struct VoteBroadcasterHandle(VoteBroadcaster);

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_broadcaster_create(
    rep_register: &RepresentativeRegisterHandle,
    tcp_channels: &TcpChannelsHandle,
    vote_processor_queue: &VoteProcessorQueueHandle,
    network_constants: &NetworkConstantsDto,
    stats: &StatHandle,
    async_rt: &AsyncRuntimeHandle,
    node_id: *const u8,
    local_endpoint: &EndpointDto,
    inbound_callback: FfiInboundCallback,
    inbound_context: *mut c_void,
    inbound_context_delete: VoidPointerCallback,
) -> *mut VoteBroadcasterHandle {
    let network_constants = NetworkConstants::try_from(network_constants).unwrap();
    let node_id = Account::from_ptr(node_id);

    let context = ContextWrapper::new(inbound_context, inbound_context_delete);
    let inbound = Arc::new(move |msg: DeserializedMessage, channel| {
        let context = context.get_context();
        inbound_callback(
            context,
            MessageHandle::new(msg),
            ChannelHandle::new(channel),
        );
    });

    Box::into_raw(Box::new(VoteBroadcasterHandle(VoteBroadcaster {
        representative_register: Arc::clone(rep_register),
        tcp_channels: Arc::clone(tcp_channels),
        vote_processor_queue: Arc::clone(vote_processor_queue),
        network_constants,
        stats: Arc::clone(stats),
        async_rt: Arc::clone(async_rt),
        node_id,
        local_endpoint: local_endpoint.into(),
        inbound,
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
