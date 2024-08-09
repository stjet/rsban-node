use super::{bandwidth_limiter::OutboundBandwidthLimiterHandle, EndpointDto, NetworkFilterHandle};
use crate::{
    messages::MessageHandle,
    utils::{AsyncRuntimeHandle, ContextWrapper},
    NetworkConstantsDto, StatHandle, VoidPointerCallback,
};
use rsnano_core::{
    utils::{system_time_as_nanoseconds, system_time_from_nanoseconds, NULL_ENDPOINT},
    Account,
};
use rsnano_messages::DeserializedMessage;
use rsnano_node::{
    config::NetworkConstants,
    transport::{Channel, ChannelEnum, ChannelTcp},
};
use std::{ffi::c_void, ops::Deref, sync::Arc, time::SystemTime};

pub struct ChannelHandle(Arc<ChannelEnum>);

impl ChannelHandle {
    pub fn new(channel: Arc<ChannelEnum>) -> *mut Self {
        Box::into_raw(Box::new(Self(channel)))
    }
}

impl Deref for ChannelHandle {
    type Target = Arc<ChannelEnum>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub unsafe fn as_tcp_channel(handle: *mut ChannelHandle) -> &'static Arc<ChannelTcp> {
    match (*handle).0.as_ref() {
        ChannelEnum::Tcp(tcp) => tcp,
        _ => panic!("expected tcp channel"),
    }
}

pub unsafe fn as_channel(handle: *mut ChannelHandle) -> &'static dyn Channel {
    (*handle).0.deref().deref()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_type(handle: *mut ChannelHandle) -> u8 {
    (*handle).0.get_type() as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_destroy(handle: *mut ChannelHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub extern "C" fn rsn_channel_close(handle: &mut ChannelHandle) {
    handle.close()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_get_last_packet_received(handle: *mut ChannelHandle) -> u64 {
    system_time_as_nanoseconds(as_channel(handle).get_last_packet_received())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_get_last_packet_sent(handle: *mut ChannelHandle) -> u64 {
    system_time_as_nanoseconds(as_channel(handle).get_last_packet_sent())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_last_packet_sent(handle: *mut ChannelHandle) {
    as_channel(handle).set_last_packet_sent(SystemTime::now());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_last_packet_sent2(handle: *mut ChannelHandle, time: u64) {
    as_channel(handle).set_last_packet_sent(system_time_from_nanoseconds(time));
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
    as_channel(handle).channel_id().as_usize()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_peering_endpoint(
    handle: &ChannelHandle,
    result: *mut EndpointDto,
) {
    (*result) = handle.peering_endpoint().unwrap_or(NULL_ENDPOINT).into()
}
