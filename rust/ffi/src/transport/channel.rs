use crate::{
    messages::MessageHandle,
    utils::{ContextWrapper, FfiIoContext},
    NetworkConstantsDto, StatHandle, VoidPointerCallback,
};

use num_traits::FromPrimitive;
use rsnano_core::Account;
use rsnano_node::{
    config::NetworkConstants,
    transport::{Channel, ChannelEnum, ChannelFake, ChannelInProc, ChannelTcp, TrafficType},
};
use std::{ffi::c_void, ops::Deref, sync::Arc};

use super::{
    bandwidth_limiter::OutboundBandwidthLimiterHandle, ChannelTcpSendBufferCallback, EndpointDto,
    NetworkFilterHandle, SendBufferCallbackWrapper,
};

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

pub type InboundCallback =
    unsafe extern "C" fn(*mut c_void, *mut MessageHandle, *mut ChannelHandle);

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_inproc_create(
    channel_id: usize,
    now: u64,
    network_constants: *const NetworkConstantsDto,
    network_filter: *mut NetworkFilterHandle,
    stats: *mut StatHandle,
    limiter: *mut OutboundBandwidthLimiterHandle,
    source_inbound_callback: InboundCallback,
    source_inbound_context: *mut c_void,
    destination_inbound_callback: InboundCallback,
    destination_inbound_context: *mut c_void,
    delete_context: VoidPointerCallback,
    io_context: *mut c_void,
    source_endpoint: *const EndpointDto,
    destination_endpoint: *const EndpointDto,
    source_node_id: *const u8,
    destination_node_id: *const u8,
) -> *mut ChannelHandle {
    let network_constants = NetworkConstants::try_from(&*network_constants).unwrap();
    let network_filter = (*network_filter).deref().clone();
    let source_context = ContextWrapper::new(source_inbound_context, delete_context);
    let source_inbound = Arc::new(move |msg, channel| {
        let context = source_context.get_context();
        source_inbound_callback(
            context,
            MessageHandle::new(msg),
            ChannelHandle::new(channel),
        );
    });
    let destination_context = ContextWrapper::new(destination_inbound_context, delete_context);
    let destination_inbound = Arc::new(move |msg, channel| {
        let context = destination_context.get_context();
        destination_inbound_callback(
            context,
            MessageHandle::new(msg),
            ChannelHandle::new(channel),
        );
    });
    ChannelHandle::new(Arc::new(ChannelEnum::InProc(ChannelInProc::new(
        channel_id,
        now,
        network_constants,
        network_filter,
        (*stats).0.clone(),
        (*limiter).0.clone(),
        source_inbound,
        destination_inbound,
        Arc::new(FfiIoContext::new(io_context)),
        (&*source_endpoint).into(),
        (&*destination_endpoint).into(),
        Account::from_ptr(source_node_id),
        Account::from_ptr(destination_node_id),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_inproc_network_version(handle: *mut ChannelHandle) -> u8 {
    let inproc = as_inproc_channel(handle);
    inproc.network_version()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_inproc_endpoint(
    handle: *mut ChannelHandle,
    result: *mut EndpointDto,
) {
    let inproc = as_inproc_channel(handle);
    (*result) = inproc.source_endpoint.into()
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
    callback: ChannelTcpSendBufferCallback,
    delete_callback: VoidPointerCallback,
    callback_context: *mut c_void,
    policy: u8,
    traffic_type: u8,
) {
    let buffer = Arc::new(std::slice::from_raw_parts(buffer, buffer_len).to_vec());
    let callback_wrapper =
        SendBufferCallbackWrapper::new(callback, callback_context, delete_callback);
    let cb = Box::new(move |ec, size| {
        callback_wrapper.call(ec, size);
    });
    let policy = FromPrimitive::from_u8(policy).unwrap();
    let traffic_type = TrafficType::from_u8(traffic_type).unwrap();
    as_inproc_channel(handle).send_buffer_2(&buffer, Some(cb), policy, traffic_type);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_inproc_send(
    handle: *mut ChannelHandle,
    message: *const MessageHandle,
    callback: ChannelTcpSendBufferCallback,
    delete_callback: VoidPointerCallback,
    callback_context: *mut c_void,
    policy: u8,
    traffic_type: u8,
) {
    let callback_wrapper =
        SendBufferCallbackWrapper::new(callback, callback_context, delete_callback);
    let cb = Box::new(move |ec, size| {
        callback_wrapper.call(ec, size);
    });
    let policy = FromPrimitive::from_u8(policy).unwrap();
    let traffic_type = TrafficType::from_u8(traffic_type).unwrap();
    as_inproc_channel(handle).send((*message).0.as_ref(), Some(cb), policy, traffic_type);
}
