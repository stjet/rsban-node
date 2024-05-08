mod websocket_server;
use crate::{
    consensus::{ElectionStatusHandle, VoteHandle, VoteWithWeightInfoVecHandle},
    core::BlockHandle,
    messages::TelemetryDataHandle,
    to_rust_string,
    transport::EndpointDto,
    utils::AsyncRuntimeHandle,
    wallets::LmdbWalletsHandle,
    StringVecHandle,
};
use num::FromPrimitive;
use rsnano_core::{Account, Amount, BlockHash, WorkVersion};
use rsnano_node::websocket::{
    MessageBuilder, OutgoingMessageEnvelope, Topic, WebsocketListener, WebsocketListenerExt,
};
use std::{
    ops::{Deref, DerefMut},
    os::raw::c_char,
    sync::Arc,
    time::Duration,
};

pub struct WebsocketMessageHandle(OutgoingMessageEnvelope);

impl WebsocketMessageHandle {
    pub fn new(envelope: OutgoingMessageEnvelope) -> *mut Self {
        Box::into_raw(Box::new(Self(envelope)))
    }
}

impl Deref for WebsocketMessageHandle {
    type Target = OutgoingMessageEnvelope;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WebsocketMessageHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_message_clone(
    handle: &WebsocketMessageHandle,
) -> *mut WebsocketMessageHandle {
    WebsocketMessageHandle::new(handle.0.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_message_destroy(handle: *mut WebsocketMessageHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_telemetry_received(
    telemetry_data: &TelemetryDataHandle,
    endpoint: &EndpointDto,
) -> *mut WebsocketMessageHandle {
    let message = MessageBuilder::telemetry_received(telemetry_data, endpoint.into()).unwrap();
    WebsocketMessageHandle::new(message)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_new_block_arrived(
    block: &BlockHandle,
) -> *mut WebsocketMessageHandle {
    let message = MessageBuilder::new_block_arrived(&**block).unwrap();
    WebsocketMessageHandle::new(message)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_started_election(
    hash: *const u8,
) -> *mut WebsocketMessageHandle {
    let message = MessageBuilder::started_election(&BlockHash::from_ptr(hash)).unwrap();
    WebsocketMessageHandle::new(message)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_stopped_election(
    hash: *const u8,
) -> *mut WebsocketMessageHandle {
    let message = MessageBuilder::stopped_election(&BlockHash::from_ptr(hash)).unwrap();
    WebsocketMessageHandle::new(message)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_vote_received(
    vote: &VoteHandle,
    vote_code: u8,
) -> *mut WebsocketMessageHandle {
    let message =
        MessageBuilder::vote_received(vote, FromPrimitive::from_u8(vote_code).unwrap()).unwrap();
    WebsocketMessageHandle::new(message)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_work_generation(
    version: u8,
    root: *const u8,
    work: u64,
    difficulty: u64,
    publish_threshold: u64,
    duration_ms: u64,
    peer: *const c_char,
    bad_peers: &StringVecHandle,
    completed: bool,
    cancelled: bool,
) -> *mut WebsocketMessageHandle {
    let message = MessageBuilder::work_generation(
        WorkVersion::from_u8(version).unwrap(),
        &BlockHash::from_ptr(root),
        work,
        difficulty,
        publish_threshold,
        Duration::from_millis(duration_ms),
        &to_rust_string(peer),
        bad_peers,
        completed,
        cancelled,
    )
    .unwrap();
    WebsocketMessageHandle::new(message)
}

pub struct WebsocketListenerHandle(Arc<WebsocketListener>);

impl WebsocketListenerHandle {
    pub fn new(listener: Arc<WebsocketListener>) -> *mut Self {
        Box::into_raw(Box::new(Self(listener)))
    }
}

impl Deref for WebsocketListenerHandle {
    type Target = Arc<WebsocketListener>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_websocket_listener_create(
    endpoint: &EndpointDto,
    wallets: &LmdbWalletsHandle,
    async_rt: &AsyncRuntimeHandle,
) -> *mut WebsocketListenerHandle {
    WebsocketListenerHandle::new(Arc::new(WebsocketListener::new(
        endpoint.into(),
        Arc::clone(wallets),
        Arc::clone(async_rt),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_listener_destroy(handle: *mut WebsocketListenerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_websocket_listener_run(handle: &WebsocketListenerHandle) {
    handle.0.start();
}

#[no_mangle]
pub extern "C" fn rsn_websocket_listener_stop(handle: &WebsocketListenerHandle) {
    handle.0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_listener_broadcast_confirmation(
    handle: &WebsocketListenerHandle,
    block: &BlockHandle,
    account: *const u8,
    amount: *const u8,
    subtype: *const c_char,
    election_status: &ElectionStatusHandle,
    election_votes: &VoteWithWeightInfoVecHandle,
) {
    handle.0.broadcast_confirmation(
        block,
        &Account::from_ptr(account),
        &Amount::from_ptr(amount),
        &to_rust_string(subtype),
        election_status,
        election_votes,
    );
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_listener_broadcast(
    handle: &WebsocketListenerHandle,
    message: &WebsocketMessageHandle,
) {
    let _ = handle.0.broadcast(message);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_listener_listening_port(
    handle: &WebsocketListenerHandle,
) -> u16 {
    handle.0.listening_port()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_listener_subscriber_count(
    handle: &WebsocketListenerHandle,
    topic: u8,
) -> usize {
    handle.0.subscriber_count(Topic::from_u8(topic).unwrap())
}
