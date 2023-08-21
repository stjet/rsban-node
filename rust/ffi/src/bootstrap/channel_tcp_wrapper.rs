use std::sync::Arc;

use crate::transport::{ChannelHandle, SocketHandle};
use rsnano_node::bootstrap::ChannelTcpWrapper;

use super::bootstrap_server::TcpServerHandle;

pub struct ChannelTcpWrapperHandle(pub Arc<ChannelTcpWrapper>);

impl ChannelTcpWrapperHandle {
    pub fn new(wrapper: Arc<ChannelTcpWrapper>) -> *mut ChannelTcpWrapperHandle {
        Box::into_raw(Box::new(ChannelTcpWrapperHandle(wrapper)))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_wrapper_create(
    channel: *mut ChannelHandle,
    socket: *mut SocketHandle,
    response_server: *mut TcpServerHandle,
) -> *mut ChannelTcpWrapperHandle {
    let response_server = if response_server.is_null() {
        None
    } else {
        Some(Arc::clone(&*response_server))
    };

    ChannelTcpWrapperHandle::new(Arc::new(ChannelTcpWrapper::new(
        Arc::clone(&(*channel).0),
        Arc::clone(&*socket),
        response_server,
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_wrapper_destroy(handle: *mut ChannelTcpWrapperHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_wrapper_get_channel(
    handle: *mut ChannelTcpWrapperHandle,
) -> *mut ChannelHandle {
    ChannelHandle::new(Arc::clone(&(*handle).0.channel))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_wrapper_get_server(
    handle: *mut ChannelTcpWrapperHandle,
) -> *mut TcpServerHandle {
    match &(*handle).0.response_server {
        Some(server) => TcpServerHandle::new(Arc::clone(server)),
        None => std::ptr::null_mut(),
    }
}
