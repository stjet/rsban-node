use super::{vote_processor_queue::VoteProcessorQueueHandle, LocalVoteHistoryHandle};
use crate::{
    ledger::datastore::LedgerHandle,
    messages::MessageHandle,
    representatives::RepresentativeRegisterHandle,
    transport::{ChannelHandle, EndpointDto, FfiInboundCallback, TcpChannelsHandle},
    utils::{AsyncRuntimeHandle, ContextWrapper},
    wallets::LmdbWalletsHandle,
    NetworkConstantsDto, StatHandle, VoidPointerCallback,
};
use rsnano_core::{Account, BlockHash, Root};
use rsnano_messages::DeserializedMessage;
use rsnano_node::{config::NetworkConstants, consensus::VoteGenerator};
use std::{
    ffi::c_void,
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Duration,
};

pub struct VoteGeneratorHandle(pub Arc<VoteGenerator>);

impl Deref for VoteGeneratorHandle {
    type Target = Arc<VoteGenerator>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for VoteGeneratorHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_generator_create(
    ledger: &LedgerHandle,
    wallets: &LmdbWalletsHandle,
    history: &LocalVoteHistoryHandle,
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
    vote_generator_delay_ms: u64,
    vote_generator_threshold: usize,
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
    Box::into_raw(Box::new(VoteGeneratorHandle(Arc::new(VoteGenerator::new(
        Arc::clone(ledger),
        Arc::clone(wallets),
        Arc::clone(history),
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
        Duration::from_millis(vote_generator_delay_ms),
        vote_generator_threshold,
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_generator_destroy(handle: *mut VoteGeneratorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_vote_generator_start(handle: &mut VoteGeneratorHandle) {
    handle.start();
}

#[no_mangle]
pub extern "C" fn rsn_vote_generator_stop(handle: &mut VoteGeneratorHandle) {
    handle.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_generator_add(
    handle: &VoteGeneratorHandle,
    root: *const u8,
    hash: *const u8,
) {
    handle.add(&Root::from_ptr(root), &BlockHash::from_ptr(hash));
}
