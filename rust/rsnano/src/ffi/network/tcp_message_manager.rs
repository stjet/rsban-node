use crate::network::TcpMessageManager;
use std::sync::Arc;

pub struct TcpMessageManagerHandle(Arc<TcpMessageManager>);

#[no_mangle]
pub extern "C" fn rsn_tcp_message_manager_create(
    incoming_connections_max: usize,
) -> *mut TcpMessageManagerHandle {
    Box::into_raw(Box::new(TcpMessageManagerHandle(Arc::new(
        TcpMessageManager::new(incoming_connections_max),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_message_manager_destroy(handle: *mut TcpMessageManagerHandle) {
    drop(Box::from_raw(handle))
}
