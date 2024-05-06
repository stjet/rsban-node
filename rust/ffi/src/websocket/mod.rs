mod options;

use crate::{
    consensus::{ElectionStatusHandle, VoteHandle, VoteWithWeightInfoVecHandle},
    core::BlockHandle,
    messages::TelemetryDataHandle,
    to_rust_string,
    transport::EndpointDto,
    wallets::LmdbWalletsHandle,
    StringVecHandle,
};

use self::options::WebsocketOptionsHandle;
use super::{FfiPropertyTree, StringDto, StringHandle};
use num::FromPrimitive;
use rsnano_core::{
    utils::{PropertyTree, SerdePropertyTree},
    Account, Amount, BlockHash, WorkVersion,
};
use rsnano_node::websocket::{
    to_topic, ConfirmationOptions, Message, MessageBuilder, Options, Topic, WebsocketSession,
};
use std::{
    collections::HashMap,
    ffi::{c_void, CStr, CString},
    net::SocketAddr,
    os::raw::c_char,
    sync::{Arc, MutexGuard},
    time::Duration,
};
use tracing::info;

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

type ListenerBroadcastCallback = unsafe extern "C" fn(*mut c_void, *const MessageDto) -> bool;
static mut BROADCAST_CALLBACK: Option<ListenerBroadcastCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_listener_broadcast(f: ListenerBroadcastCallback) {
    BROADCAST_CALLBACK = Some(f);
    rsnano_node::websocket::BROADCAST_CALLBACK = Some(|cpp_pointer, message| {
        let message_dto = MessageDto {
            topic: message.topic as u8,
            contents: message
                .contents
                .as_any()
                .downcast_ref::<FfiPropertyTree>()
                .ok_or_else(|| anyhow!("not an FfiPropertyTreeWriter"))?
                .handle,
        };
        if (BROADCAST_CALLBACK.unwrap())(cpp_pointer, &message_dto) {
            Ok(())
        } else {
            Err(anyhow!("callback failed"))
        }
    });
}

pub struct WebsocketSessionHandle(Arc<WebsocketSession>);

#[no_mangle]
pub extern "C" fn rsn_websocket_session_create() -> *mut WebsocketSessionHandle {
    Box::into_raw(Box::new(WebsocketSessionHandle(Arc::new(
        WebsocketSession::new(),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_session_destroy(handle: *mut WebsocketSessionHandle) {
    drop(Box::from_raw(handle))
}

pub struct ListenerSubscriptionsLock(MutexGuard<'static, HashMap<Topic, Options>>);

#[no_mangle]
pub extern "C" fn rsn_websocket_session_lock_subscriptions(
    handle: &WebsocketSessionHandle,
) -> *mut ListenerSubscriptionsLock {
    let guard = handle.0.subscriptions.lock().unwrap();
    let guard = unsafe {
        std::mem::transmute::<
            MutexGuard<HashMap<Topic, Options>>,
            MutexGuard<'static, HashMap<Topic, Options>>,
        >(guard)
    };
    Box::into_raw(Box::new(ListenerSubscriptionsLock(guard)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_listener_subscriptions_lock_destroy(
    handle: *mut ListenerSubscriptionsLock,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_listener_subscriptions_lock_get_topics(
    handle: &ListenerSubscriptionsLock,
) -> *mut TopicVecHandle {
    Box::into_raw(Box::new(TopicVecHandle(handle.0.keys().cloned().collect())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_listener_subscriptions_lock_contains_topic(
    handle: &ListenerSubscriptionsLock,
    topic: u8,
) -> bool {
    handle.0.contains_key(&Topic::from_u8(topic).unwrap())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_listener_subscriptions_lock_get_conf_opts_or_default(
    handle: &ListenerSubscriptionsLock,
    topic: u8,
    wallets: &LmdbWalletsHandle,
) -> *mut WebsocketOptionsHandle {
    let default_options = Options::Confirmation(ConfirmationOptions::new(
        Arc::clone(wallets),
        &SerdePropertyTree::new(),
    ));

    let options = match handle.0.get(&Topic::from_u8(topic).unwrap()) {
        Some(i) => match i {
            Options::Confirmation(_) => i.clone(),
            _ => default_options,
        },
        None => default_options,
    };

    Box::into_raw(Box::new(WebsocketOptionsHandle(options)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_listener_subscriptions_lock_should_filter(
    handle: &ListenerSubscriptionsLock,
    message: &MessageDto,
) -> bool {
    let message = Message::from(message);
    if let Some(options) = handle.0.get(&message.topic) {
        options.should_filter(&message)
    } else {
        false
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_listener_subscriptions_lock_set_options(
    handle: &mut ListenerSubscriptionsLock,
    topic: u8,
    options: &mut WebsocketOptionsHandle,
    remote: &EndpointDto,
) -> bool {
    let topic = Topic::from_u8(topic).unwrap();
    let endpoint = SocketAddr::from(remote);
    match handle.0.insert(topic, options.0.clone()) {
        Some(_) => {
            info!(
                "Updated subscription to topic: {} ({})",
                topic.as_str(),
                endpoint
            );
            false
        }
        None => {
            info!(
                "New subscription to topic: {} ({})",
                topic.as_str(),
                endpoint
            );
            true
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_listener_subscriptions_lock_update(
    handle: &mut ListenerSubscriptionsLock,
    topic: u8,
    message: *mut c_void,
) -> bool {
    let topic = Topic::from_u8(topic).unwrap();
    let message = FfiPropertyTree::new_borrowed(message);
    if let Some(option) = handle.0.get_mut(&topic) {
        if let Some(options_text) = message.get_child("options") {
            if option.update(&*options_text) {
                return true;
            }
        }
    }
    false
}

#[no_mangle]
pub unsafe extern "C" fn rsn_listener_subscriptions_lock_remove(
    handle: &mut ListenerSubscriptionsLock,
    topic: u8,
) -> bool {
    let topic = Topic::from_u8(topic).unwrap();
    handle.0.remove(&topic).is_some()
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
