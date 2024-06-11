use super::{
    ChannelHandle, EndpointDto, NetworkFilterHandle, OutboundBandwidthLimiterHandle,
    SocketFfiObserver, SynCookiesHandle, TcpMessageManagerHandle,
};
use crate::{
    messages::MessageHandle,
    utils::{AsyncRuntimeHandle, ThreadPoolHandle},
    NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle,
};
use rsnano_core::{utils::system_time_from_nanoseconds, KeyPair, PublicKey};
use rsnano_node::{
    config::NodeConfig,
    transport::{ChannelEnum, ChannelMode, Network, NetworkExt, NetworkOptions},
    NetworkParams,
};
use std::{
    ffi::c_void,
    net::{Ipv6Addr, SocketAddrV6},
    ops::Deref,
    sync::Arc,
};

pub struct TcpChannelsHandle(pub Arc<Network>);

impl Deref for TcpChannelsHandle {
    type Target = Arc<Network>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(C)]
pub struct TcpChannelsOptionsDto {
    pub node_config: *const NodeConfigDto,
    pub publish_filter: *mut NetworkFilterHandle,
    pub async_rt: *mut AsyncRuntimeHandle,
    pub network: *mut NetworkParamsDto,
    pub stats: *mut StatHandle,
    pub tcp_message_manager: *mut TcpMessageManagerHandle,
    pub port: u16,
    pub flags: *mut NodeFlagsHandle,
    pub limiter: *mut OutboundBandwidthLimiterHandle,
    pub node_id_prv: *const u8,
    pub syn_cookies: *mut SynCookiesHandle,
    pub workers: *mut ThreadPoolHandle,
    pub socket_observer: *mut c_void,
}

impl TryFrom<&TcpChannelsOptionsDto> for NetworkOptions {
    type Error = anyhow::Error;

    fn try_from(value: &TcpChannelsOptionsDto) -> Result<Self, Self::Error> {
        unsafe {
            let observer = Arc::new(SocketFfiObserver::new(value.socket_observer));

            Ok(Self {
                node_config: NodeConfig::try_from(&*value.node_config)?,
                publish_filter: (*value.publish_filter).0.clone(),
                network_params: NetworkParams::try_from(&*value.network)?,
                async_rt: Arc::clone(&(*value.async_rt).0),
                stats: (*value.stats).0.clone(),
                tcp_message_manager: (*value.tcp_message_manager).deref().clone(),
                port: value.port,
                flags: (*value.flags).0.lock().unwrap().clone(),
                limiter: (*value.limiter).0.clone(),
                node_id: KeyPair::from_priv_key_bytes(std::slice::from_raw_parts(
                    value.node_id_prv,
                    32,
                ))
                .unwrap(),
                syn_cookies: (*value.syn_cookies).0.clone(),
                workers: (*value.workers).0.clone(),
                observer,
            })
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_create(
    options: &TcpChannelsOptionsDto,
) -> *mut TcpChannelsHandle {
    let channels = Arc::new(Network::new(NetworkOptions::try_from(options).unwrap()));
    Box::into_raw(Box::new(TcpChannelsHandle(channels)))
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_port(handle: &TcpChannelsHandle) -> u16 {
    handle.port()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_destroy(handle: *mut TcpChannelsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_purge(handle: &TcpChannelsHandle, cutoff_ns: u64) {
    let cutoff = system_time_from_nanoseconds(cutoff_ns);
    handle.purge(cutoff);
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_channel_count(handle: &mut TcpChannelsHandle) -> usize {
    handle.count_by_mode(ChannelMode::Realtime)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_not_a_peer(
    handle: &mut TcpChannelsHandle,
    endpoint: &EndpointDto,
    allow_local_peers: bool,
) -> bool {
    handle.not_a_peer(&endpoint.into(), allow_local_peers)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_find_channel(
    handle: &mut TcpChannelsHandle,
    endpoint: &EndpointDto,
) -> *mut ChannelHandle {
    match handle.find_channel(&endpoint.into()) {
        Some(channel) => ChannelHandle::new(channel),
        None => std::ptr::null_mut(),
    }
}
#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_random_channels(
    handle: &mut TcpChannelsHandle,
    count: usize,
    min_version: u8,
) -> *mut ChannelListHandle {
    let channels = handle.random_channels(count, min_version);
    Box::into_raw(Box::new(ChannelListHandle(channels)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_dump(handle: &TcpChannelsHandle) {
    handle.dump_channels()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_find_node_id(
    handle: &mut TcpChannelsHandle,
    node_id: *const u8,
) -> *mut ChannelHandle {
    let node_id = PublicKey::from_ptr(node_id);
    match handle.find_node_id(&node_id) {
        Some(channel) => ChannelHandle::new(channel),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_random_fill(
    handle: &TcpChannelsHandle,
    endpoints: *mut EndpointDto,
) {
    let endpoints = std::slice::from_raw_parts_mut(endpoints, 8);
    let null_endpoint = SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0);
    let mut tmp = [null_endpoint; 8];
    handle.random_fill(&mut tmp);
    endpoints
        .iter_mut()
        .zip(&tmp)
        .for_each(|(dto, ep)| *dto = ep.into());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_get_next_channel_id(handle: &TcpChannelsHandle) -> usize {
    handle.get_next_channel_id()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_reachout(
    handle: &TcpChannelsHandle,
    endpoint: &EndpointDto,
) -> bool {
    handle.track_reachout(&endpoint.into())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_len_sqrt(handle: &TcpChannelsHandle) -> f32 {
    handle.len_sqrt()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_fanout(handle: &TcpChannelsHandle, scale: f32) -> usize {
    handle.fanout(scale)
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_random_fanout(
    handle: &TcpChannelsHandle,
    scale: f32,
) -> *mut ChannelListHandle {
    let channels = handle.random_fanout(scale);
    Box::into_raw(Box::new(ChannelListHandle(channels)))
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_merge_peer(handle: &TcpChannelsHandle, peer: &EndpointDto) {
    handle.merge_peer(peer.into());
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_flood_message(
    handle: &TcpChannelsHandle,
    msg: &MessageHandle,
    scale: f32,
) {
    handle.flood_message(&msg.message, scale)
}

pub struct ChannelListHandle(Vec<Arc<ChannelEnum>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_list_len(handle: *mut ChannelListHandle) -> usize {
    (*handle).0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_list_get(
    handle: *mut ChannelListHandle,
    index: usize,
) -> *mut ChannelHandle {
    ChannelHandle::new((*handle).0[index].clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_list_destroy(handle: *mut ChannelListHandle) {
    drop(Box::from_raw(handle))
}
