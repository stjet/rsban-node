use crate::{NetworkConstantsDto, VoidPointerCallback};

use rsnano_core::Account;
use rsnano_node::{
    config::NetworkConstants,
    transport::{Channel, ChannelEnum, ChannelFake, ChannelInProc, ChannelTcp},
};
use std::{ffi::c_void, ops::Deref, sync::Arc};

use super::{MessageCallbackWrapper, MessageReceivedCallback, NetworkFilterHandle};

pub struct ChannelHandle(pub Arc<ChannelEnum>);

impl ChannelHandle {
    pub fn new(channel: Arc<ChannelEnum>) -> *mut Self {
        Box::into_raw(Box::new(Self(channel)))
    }
}

pub unsafe fn as_inproc_channel(handle: *mut ChannelHandle) -> &'static ChannelInProc {
    match (*handle).0.as_ref() {
        ChannelEnum::InProc(inproc) => inproc,
        _ => panic!("expected inproc channel"),
    }
}

pub unsafe fn as_tcp_channel(handle: *mut ChannelHandle) -> &'static ChannelTcp {
    match (*handle).0.as_ref() {
        ChannelEnum::Tcp(tcp) => tcp,
        _ => panic!("expected tcp channel"),
    }
}

pub unsafe fn as_channel(handle: *mut ChannelHandle) -> &'static dyn Channel {
    (*handle).0.as_channel()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_type(handle: *mut ChannelHandle) -> u8 {
    (*handle).0.as_channel().get_type() as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_destroy(handle: *mut ChannelHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_is_temporary(handle: *mut ChannelHandle) -> bool {
    as_channel(handle).is_temporary()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_temporary(handle: *mut ChannelHandle, temporary: bool) {
    as_channel(handle).set_temporary(temporary);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_get_last_bootstrap_attempt(handle: *mut ChannelHandle) -> u64 {
    as_channel(handle).get_last_bootstrap_attempt()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_last_bootstrap_attempt(
    handle: *mut ChannelHandle,
    instant: u64,
) {
    as_channel(handle).set_last_bootstrap_attempt(instant);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_get_last_packet_received(handle: *mut ChannelHandle) -> u64 {
    as_channel(handle).get_last_packet_received()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_last_packet_received(
    handle: *mut ChannelHandle,
    instant: u64,
) {
    as_channel(handle).set_last_packet_received(instant);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_get_last_packet_sent(handle: *mut ChannelHandle) -> u64 {
    as_channel(handle).get_last_packet_sent()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_last_packet_sent(
    handle: *mut ChannelHandle,
    instant: u64,
) {
    as_channel(handle).set_last_packet_sent(instant);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_get_node_id(
    handle: *mut ChannelHandle,
    result: *mut u8,
) -> bool {
    match as_channel(handle).get_node_id() {
        Some(id) => {
            std::slice::from_raw_parts_mut(result, 32).copy_from_slice(id.as_bytes());
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_node_id(handle: *mut ChannelHandle, id: *const u8) {
    as_channel(handle).set_node_id(Account::from_ptr(id));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_id(handle: *mut ChannelHandle) -> usize {
    as_channel(handle).channel_id()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_inproc_create(
    channel_id: usize,
    now: u64,
    network_constants: *const NetworkConstantsDto,
    network_filter: *mut NetworkFilterHandle,
) -> *mut ChannelHandle {
    let network_constants = NetworkConstants::try_from(&*network_constants).unwrap();
    let network_filter = (*network_filter).deref().clone();
    ChannelHandle::new(Arc::new(ChannelEnum::InProc(ChannelInProc::new(
        channel_id,
        now,
        network_constants,
        network_filter,
    ))))
}

#[no_mangle]
pub extern "C" fn rsn_channel_fake_create(now: u64, channel_id: usize) -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelEnum::Fake(
        ChannelFake::new(now, channel_id),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_inproc_send_buffer(
    handle: *mut ChannelHandle,
    buffer: *const u8,
    buffer_len: usize,
    message_callback: MessageReceivedCallback,
    message_callback_context: *mut c_void,
    delete_callback_context: VoidPointerCallback,
) {
    let buffer = std::slice::from_raw_parts(buffer, buffer_len);

    let message_callback_wrapper = MessageCallbackWrapper::new(
        message_callback,
        message_callback_context,
        delete_callback_context,
    );

    let message_received = Box::new(move |ec, msg| {
        message_callback_wrapper.call(ec, msg);
    });

    as_inproc_channel(handle).send_buffer(buffer, message_received);
}
