use super::{
    bandwidth_limiter::OutboundBandwidthLimiterHandle,
    channel::{as_tcp_channel, ChannelHandle},
    socket::SocketHandle,
    EndpointDto, TcpChannelsHandle,
};
use crate::{utils::AsyncRuntimeHandle, NetworkConstantsDto, StatHandle};
use rsnano_node::{
    config::NetworkConstants,
    transport::{Channel, ChannelEnum, ChannelTcp},
};
use std::{ops::Deref, sync::Arc, time::SystemTime};

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_create(
    socket: *mut SocketHandle,
    stats: &StatHandle,
    _tcp_channels: &TcpChannelsHandle,
    limiter: *const OutboundBandwidthLimiterHandle,
    async_rt: &AsyncRuntimeHandle,
    channel_id: usize,
    network_constants: &NetworkConstantsDto,
) -> *mut ChannelHandle {
    let limiter = Arc::clone(&*limiter);
    let async_rt = Arc::clone(&async_rt.0);
    let protocol = NetworkConstants::try_from(network_constants)
        .unwrap()
        .protocol_info();

    ChannelHandle::new(Arc::new(ChannelEnum::Tcp(Arc::new(ChannelTcp::new(
        Arc::clone((*socket).deref()),
        SystemTime::now(),
        Arc::clone(stats),
        limiter,
        &async_rt,
        channel_id.into(),
        protocol,
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_local_endpoint(
    handle: *mut ChannelHandle,
    endpoint: *mut EndpointDto,
) {
    (*endpoint) = EndpointDto::from(as_tcp_channel(handle).local_endpoint());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_remote_endpoint(
    handle: *mut ChannelHandle,
    endpoint: *mut EndpointDto,
) {
    (*endpoint) = EndpointDto::from(as_tcp_channel(handle).remote_addr())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_socket_id(handle: *mut ChannelHandle) -> usize {
    as_tcp_channel(handle).socket_id()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_network_version(handle: *mut ChannelHandle) -> u8 {
    let tcp = as_tcp_channel(handle);
    tcp.network_version()
}
