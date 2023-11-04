use super::socket::EndpointDto;
use crate::copy_account_bytes;
use rsnano_node::transport::TcpMessageItem;
use std::ops::Deref;

pub struct TcpMessageItemHandle(TcpMessageItem);

impl TcpMessageItemHandle {
    pub fn new(msg: TcpMessageItem) -> *mut Self {
        Box::into_raw(Box::new(TcpMessageItemHandle(msg)))
    }
}

impl Deref for TcpMessageItemHandle {
    type Target = TcpMessageItem;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_tcp_message_item_empty() -> *mut TcpMessageItemHandle {
    TcpMessageItemHandle::new(TcpMessageItem::new())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_message_item_clone(
    handle: *mut TcpMessageItemHandle,
) -> *mut TcpMessageItemHandle {
    Box::into_raw(Box::new(TcpMessageItemHandle((*handle).0.clone())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_message_item_destroy(handle: *mut TcpMessageItemHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_message_item_endpoint(
    handle: *mut TcpMessageItemHandle,
    endpoint: *mut EndpointDto,
) {
    *endpoint = EndpointDto::from(&(*handle).0.endpoint);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_message_item_node_id(
    handle: *mut TcpMessageItemHandle,
    node_id: *mut u8,
) {
    copy_account_bytes((*handle).0.node_id, node_id);
}
