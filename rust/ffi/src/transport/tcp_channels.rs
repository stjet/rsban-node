use std::{
    net::{Ipv6Addr, SocketAddr, SocketAddrV6},
    sync::{Arc, Mutex},
};

use rsnano_core::{
    utils::{system_time_as_nanoseconds, system_time_from_nanoseconds},
    PublicKey,
};
use rsnano_node::transport::{TcpChannels, TcpEndpointAttempt};

use crate::utils::ptr_into_ipv6addr;

use super::EndpointDto;

pub struct TcpChannelsHandle(Arc<Mutex<TcpChannels>>);

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_create() -> *mut TcpChannelsHandle {
    Box::into_raw(Box::new(TcpChannelsHandle(Arc::new(Mutex::new(
        TcpChannels::new(),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_destroy(handle: *mut TcpChannelsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_erase_attempt(
    handle: *mut TcpChannelsHandle,
    endpoint: &EndpointDto,
) {
    (*handle)
        .0
        .lock()
        .unwrap()
        .attempts
        .remove(&endpoint.into());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_get_attempt_count_by_ip_address(
    handle: *mut TcpChannelsHandle,
    ipv6_bytes: *const u8,
) -> usize {
    (*handle)
        .0
        .lock()
        .unwrap()
        .attempts
        .count_by_address(&ptr_into_ipv6addr(ipv6_bytes))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_get_attempt_count_by_subnetwork(
    handle: *mut TcpChannelsHandle,
    ipv6_bytes: *const u8,
) -> usize {
    (*handle)
        .0
        .lock()
        .unwrap()
        .attempts
        .count_by_subnetwork(&ptr_into_ipv6addr(ipv6_bytes))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_add_attempt(
    handle: *mut TcpChannelsHandle,
    endpoint: &EndpointDto,
) -> bool {
    let attempt = TcpEndpointAttempt::new(endpoint.into());
    let mut guard = (*handle).0.lock().unwrap();
    guard.attempts.insert(attempt.into())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_attempts_count(handle: *mut TcpChannelsHandle) -> usize {
    let guard = (*handle).0.lock().unwrap();
    guard.attempts.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_attempts_purge(
    handle: *mut TcpChannelsHandle,
    cutoff_ns: u64,
) {
    let cutoff = system_time_from_nanoseconds(cutoff_ns);
    let mut guard = (*handle).0.lock().unwrap();
    guard.attempts.purge(cutoff)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_channels_erase_channel_by_node_id(
    handle: &mut TcpChannelsHandle,
    node_id: *const u8,
) {
    handle
        .0
        .lock()
        .unwrap()
        .channels
        .remove_by_node_id(&PublicKey::from_ptr(node_id))
}

#[no_mangle]
pub extern "C" fn rsn_tcp_channels_channel_exists(
    handle: &mut TcpChannelsHandle,
    endpoint: &EndpointDto,
) -> bool {
    handle
        .0
        .lock()
        .unwrap()
        .channels
        .exists(&SocketAddr::from(endpoint))
}

pub struct TcpEndpointAttemptDto {
    pub endpoint: EndpointDto,
    pub address: [u8; 16],
    pub subnetwork: [u8; 16],
    pub last_attempt: u64,
}

impl From<&TcpEndpointAttemptDto> for TcpEndpointAttempt {
    fn from(value: &TcpEndpointAttemptDto) -> Self {
        Self {
            endpoint: SocketAddrV6::from(&value.endpoint),
            address: Ipv6Addr::from(value.address),
            subnetwork: Ipv6Addr::from(value.subnetwork),
            last_attempt: system_time_from_nanoseconds(value.last_attempt),
        }
    }
}

impl From<&TcpEndpointAttempt> for TcpEndpointAttemptDto {
    fn from(value: &TcpEndpointAttempt) -> Self {
        Self {
            endpoint: value.endpoint.into(),
            address: value.address.octets(),
            subnetwork: value.subnetwork.octets(),
            last_attempt: system_time_as_nanoseconds(value.last_attempt),
        }
    }
}
