use crate::FfiPropertyTree;
use rsnano_node::bootstrap::BootstrapConnections;
use std::{ffi::c_void, ops::Deref, sync::Arc};

pub struct BootstrapConnectionsHandle(Arc<BootstrapConnections>);

impl BootstrapConnectionsHandle {
    pub fn new(connections: Arc<BootstrapConnections>) -> *mut Self {
        Box::into_raw(Box::new(Self(connections)))
    }
}

impl Deref for BootstrapConnectionsHandle {
    type Target = Arc<BootstrapConnections>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_connections_drop(handle: *mut BootstrapConnectionsHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_bootstrap_status(
    handle: &BootstrapConnectionsHandle,
    tree: *mut c_void,
    attempts_count: usize,
) {
    handle.bootstrap_status(&mut FfiPropertyTree::new_borrowed(tree), attempts_count);
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_target_connections(
    handle: &BootstrapConnectionsHandle,
    pulls_remaining: usize,
    attempts_count: usize,
) -> u32 {
    handle.target_connections(pulls_remaining, attempts_count)
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_connections_count(
    handle: &BootstrapConnectionsHandle,
) -> u32 {
    handle
        .connections_count
        .load(std::sync::atomic::Ordering::SeqCst)
}
