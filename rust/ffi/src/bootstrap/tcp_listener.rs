use std::sync::Arc;

use crate::{
    transport::{SynCookiesHandle, TcpChannelsHandle},
    utils::{LoggerHandle, LoggerMT},
    NodeConfigDto,
};
use rsnano_core::utils::Logger;
use rsnano_node::transport::TcpListener;

pub struct TcpListenerHandle(TcpListener);

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_listener_create(
    port: u16,
    max_inbound_connections: usize,
    config: &NodeConfigDto,
    logger: *mut LoggerHandle,
    tcp_channels: &TcpChannelsHandle,
    syn_cookies: &SynCookiesHandle,
) -> *mut TcpListenerHandle {
    let logger: Arc<dyn Logger> = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    Box::into_raw(Box::new(TcpListenerHandle(TcpListener::new(
        port,
        max_inbound_connections,
        config.try_into().unwrap(),
        logger,
        Arc::clone(tcp_channels),
        Arc::clone(syn_cookies),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_listener_destroy(handle: *mut TcpListenerHandle) {
    drop(Box::from_raw(handle))
}
