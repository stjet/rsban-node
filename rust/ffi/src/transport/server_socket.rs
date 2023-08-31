use rsnano_node::transport::ServerSocket;
use std::sync::Arc;

pub struct ServerSocketHandle(Arc<ServerSocket>);

#[no_mangle]
pub extern "C" fn rsn_server_socket_create() -> *mut ServerSocketHandle {
    Box::into_raw(Box::new(ServerSocketHandle(Arc::new(ServerSocket::new()))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_server_socket_destroy(handle: *mut ServerSocketHandle) {
    drop(Box::from_raw(handle))
}
