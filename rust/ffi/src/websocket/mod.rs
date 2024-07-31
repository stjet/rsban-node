use crate::{consensus::VoteHandle, to_rust_string, StringVecHandle};
use num::FromPrimitive;
use rsnano_core::{BlockHash, WorkVersion};
use rsnano_node::websocket::{
    vote_received, work_generation_message, OutgoingMessageEnvelope, Topic, WebsocketListener,
    WebsocketListenerExt,
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
pub unsafe extern "C" fn rsn_message_builder_vote_received(
    vote: &VoteHandle,
    vote_code: u8,
) -> *mut WebsocketMessageHandle {
    let message = vote_received(vote, FromPrimitive::from_u8(vote_code).unwrap());
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
    let message = work_generation_message(
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
    );
    WebsocketMessageHandle::new(message)
}

pub struct WebsocketListenerHandle(pub Arc<WebsocketListener>);

impl Deref for WebsocketListenerHandle {
    type Target = Arc<WebsocketListener>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
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
