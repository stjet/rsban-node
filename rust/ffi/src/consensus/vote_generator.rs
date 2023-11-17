use crate::{
    ledger::datastore::{LedgerHandle, TransactionHandle},
    messages::MessageHandle,
    representatives::RepresentativeRegisterHandle,
    transport::{ChannelHandle, EndpointDto, FfiInboundCallback, TcpChannelsHandle},
    utils::{AsyncRuntimeHandle, ContextWrapper},
    NetworkConstantsDto, StatHandle, VoidPointerCallback,
};
use rsnano_core::{Account, BlockHash, Root};
use rsnano_node::{
    config::NetworkConstants, consensus::VoteGenerator, messages::DeserializedMessage,
};
use std::{ffi::c_void, ops::Deref, sync::Arc, time::Duration};

use super::vote_processor_queue::VoteProcessorQueueHandle;

pub struct VoteGeneratorHandle(VoteGenerator);

impl Deref for VoteGeneratorHandle {
    type Target = VoteGenerator;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_generator_create(
    ledger: &LedgerHandle,
    is_final: bool,
    stats: &StatHandle,
    representative_register: &RepresentativeRegisterHandle,
    tcp_channels: &TcpChannelsHandle,
    vote_processor_queue: &VoteProcessorQueueHandle,
    network_constants: &NetworkConstantsDto,
    async_rt: &AsyncRuntimeHandle,
    node_id: *const u8,
    local_endpoint: &EndpointDto,
    inbound_callback: FfiInboundCallback,
    inbound_context: *mut c_void,
    inbound_context_delete: VoidPointerCallback,
    voting_delay_s: u64,
) -> *mut VoteGeneratorHandle {
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
    Box::into_raw(Box::new(VoteGeneratorHandle(VoteGenerator::new(
        Arc::clone(ledger),
        is_final,
        Arc::clone(stats),
        Arc::clone(representative_register),
        Arc::clone(tcp_channels),
        Arc::clone(vote_processor_queue),
        network_constants,
        Arc::clone(async_rt),
        node_id,
        local_endpoint.into(),
        inbound,
        Duration::from_secs(voting_delay_s),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_generator_destroy(handle: *mut VoteGeneratorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_generator_should_vote(
    handle: &VoteGeneratorHandle,
    transaction: &mut TransactionHandle,
    root: *const u8,
    hash: *const u8,
) -> bool {
    let root = Root::from_ptr(root);
    let hash = BlockHash::from_ptr(hash);
    handle.should_vote(transaction.as_write_txn(), &root, &hash)
}
