mod options;

use self::options::WebsocketOptionsHandle;
use super::{FfiPropertyTree, StringDto, StringHandle};
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
    to_topic, Listener, Message, MessageBuilder, Topic, WebsocketListener, WebsocketListenerExt,
};
use std::{
    ffi::{c_void, CStr, CString},
    ops::Deref,
    os::raw::c_char,
    sync::Arc,
    time::Duration,
};

#[repr(C)]
pub struct MessageDto {
    pub topic: u8,
    pub contents: *mut c_void,
}

impl From<&MessageDto> for Message {
    fn from(value: &MessageDto) -> Self {
        Self {
            topic: FromPrimitive::from_u8(value.topic).unwrap(),
            contents: Box::new(FfiPropertyTree::new_borrowed(value.contents)),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_set_common_fields(message: *mut MessageDto) {
    let dto = &mut (*message);
    let mut message = Message {
        topic: FromPrimitive::from_u8(dto.topic).unwrap(),
        contents: Box::new(FfiPropertyTree::new_borrowed(dto.contents)),
    };
    MessageBuilder::set_common_fields(&mut message).unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_from_topic(topic: u8, result: *mut StringDto) {
    let topic_string = Box::new(StringHandle(
        CString::new(Topic::from_u8(topic).unwrap().as_str()).unwrap(),
    ));
    (*result).value = topic_string.0.as_ptr();
    (*result).handle = Box::into_raw(topic_string);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_to_topic(topic: *const c_char) -> u8 {
    to_topic(CStr::from_ptr(topic).to_string_lossy()) as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_bootstrap_started(
    id: *const c_char,
    mode: *const c_char,
    result: *mut MessageDto,
) {
    let message = MessageBuilder::bootstrap_started(
        &CStr::from_ptr(id).to_string_lossy(),
        &CStr::from_ptr(mode).to_string_lossy(),
    )
    .unwrap();

    set_message_dto(result, message);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_bootstrap_exited(
    id: *const c_char,
    mode: *const c_char,
    duration_s: u64,
    total_blocks: u64,
    result: *mut MessageDto,
) {
    let message = MessageBuilder::bootstrap_exited(
        &CStr::from_ptr(id).to_string_lossy(),
        &CStr::from_ptr(mode).to_string_lossy(),
        Duration::from_secs(duration_s),
        total_blocks,
    )
    .unwrap();

    set_message_dto(result, message);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_telemetry_received(
    telemetry_data: &TelemetryDataHandle,
    endpoint: &EndpointDto,
    result: *mut MessageDto,
) {
    let message = MessageBuilder::telemetry_received(telemetry_data, endpoint.into()).unwrap();
    set_message_dto(result, message);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_new_block_arrived(
    block: &BlockHandle,
    result: *mut MessageDto,
) {
    let message = MessageBuilder::new_block_arrived(&**block).unwrap();
    set_message_dto(result, message);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_started_election(
    hash: *const u8,
    result: *mut MessageDto,
) {
    let message = MessageBuilder::started_election(&BlockHash::from_ptr(hash)).unwrap();
    set_message_dto(result, message);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_stopped_election(
    hash: *const u8,
    result: *mut MessageDto,
) {
    let message = MessageBuilder::stopped_election(&BlockHash::from_ptr(hash)).unwrap();
    set_message_dto(result, message);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_vote_received(
    vote: &VoteHandle,
    vote_code: u8,
    result: *mut MessageDto,
) {
    let message =
        MessageBuilder::vote_received(vote, FromPrimitive::from_u8(vote_code).unwrap()).unwrap();
    set_message_dto(result, message);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_block_confirmed(
    block: &BlockHandle,
    account: *const u8,
    amount: *const u8,
    subtype: *const c_char,
    include_block: bool,
    election_status: &ElectionStatusHandle,
    votes: &VoteWithWeightInfoVecHandle,
    options: &WebsocketOptionsHandle,
    result: *mut MessageDto,
) {
    let message = MessageBuilder::block_confirmed(
        block,
        &Account::from_ptr(account),
        &Amount::from_ptr(amount),
        to_rust_string(subtype),
        include_block,
        election_status,
        votes,
        options.confirmation_options(),
    )
    .unwrap();
    set_message_dto(result, message);
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
    result: *mut MessageDto,
) {
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
    set_message_dto(result, message);
}

unsafe fn set_message_dto(result: *mut MessageDto, mut message: Message) {
    (*result).topic = message.topic as u8;
    let ffi_ptree = message
        .contents
        .as_any_mut()
        .downcast_mut::<FfiPropertyTree>()
        .unwrap();
    (*result).contents = ffi_ptree.handle;
    // Prevent the property_tree from being deleted.
    // The caller of this function is responsable for calling delete on the handle.
    ffi_ptree.make_borrowed();
}

pub struct TopicVecHandle(Vec<Topic>);

#[no_mangle]
pub extern "C" fn rsn_topic_vec_len(handle: &TopicVecHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub extern "C" fn rsn_topic_vec_get(handle: &TopicVecHandle, index: usize) -> u8 {
    handle.0[index] as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_topic_vec_destroy(handle: *mut TopicVecHandle) {
    drop(Box::from_raw(handle))
}

pub struct WebsocketListenerHandle(Arc<WebsocketListener>);

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
    Box::into_raw(Box::new(WebsocketListenerHandle(Arc::new(
        WebsocketListener::new(endpoint.into(), Arc::clone(wallets), Arc::clone(async_rt)),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_listener_destroy(handle: *mut WebsocketListenerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_websocket_listener_run(handle: &WebsocketListenerHandle) {
    handle.0.run();
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
    message: &MessageDto,
) {
    handle.0.broadcast(&message.into());
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
