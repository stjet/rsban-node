use super::EndpointDto;
use rsnano_core::utils::system_time_from_nanoseconds;
use rsnano_node::transport::{ChannelMode, Network};
use std::{
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
    handle
        .info
        .read()
        .unwrap()
        .count_by_mode(ChannelMode::Realtime)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_random_fill(
    handle: &TcpChannelsHandle,
    endpoints: *mut EndpointDto,
) {
    let endpoints = std::slice::from_raw_parts_mut(endpoints, 8);
    let null_endpoint = SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0);
    let mut tmp = [null_endpoint; 8];
    handle.random_fill_peering_endpoints(&mut tmp);
    endpoints
        .iter_mut()
        .zip(&tmp)
        .for_each(|(dto, ep)| *dto = ep.into());
}
