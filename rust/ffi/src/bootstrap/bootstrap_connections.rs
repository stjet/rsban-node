use rsnano_node::bootstrap::{BootstrapConnections, DROP_BOOTSTRAP_CONNECTIONS_CALLBACK};
use std::{ffi::c_void, sync::Arc};

use crate::VoidPointerCallback;

pub struct BootstrapConnectionsHandle(Arc<BootstrapConnections>);

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_create(
    cpp_handle: *mut c_void,
) -> *mut BootstrapConnectionsHandle {
    Box::into_raw(Box::new(BootstrapConnectionsHandle(Arc::new(
        BootstrapConnections::new(cpp_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_connections_drop(handle: *mut BootstrapConnectionsHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_connections_dropped(callback: VoidPointerCallback) {
    unsafe {
        DROP_BOOTSTRAP_CONNECTIONS_CALLBACK = Some(callback);
    }
}
