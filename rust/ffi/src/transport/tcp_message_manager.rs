use rsnano_node::transport::MessageProcessor;
use std::{ops::Deref, sync::Arc};

pub struct TcpMessageManagerHandle(pub Arc<MessageProcessor>);

impl Deref for TcpMessageManagerHandle {
    type Target = Arc<MessageProcessor>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_tcp_message_manager_create(
    incoming_connections_max: usize,
) -> *mut TcpMessageManagerHandle {
    Box::into_raw(Box::new(TcpMessageManagerHandle(Arc::new(
        MessageProcessor::new(incoming_connections_max),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_message_manager_destroy(handle: *mut TcpMessageManagerHandle) {
    drop(Box::from_raw(handle))
}
