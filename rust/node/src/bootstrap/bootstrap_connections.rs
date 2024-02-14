use super::{BootstrapClient, PullInfo};
use std::{ffi::c_void, sync::Arc};

pub struct BootstrapConnections {
    cpp_handle: *mut c_void,
}

impl BootstrapConnections {
    pub fn new(cpp_handle: *mut c_void) -> Self {
        Self { cpp_handle }
    }
}

unsafe impl Send for BootstrapConnections {}
unsafe impl Sync for BootstrapConnections {}

pub static mut DROP_BOOTSTRAP_CONNECTIONS_CALLBACK: Option<unsafe extern "C" fn(*mut c_void)> =
    None;

impl Drop for BootstrapConnections {
    fn drop(&mut self) {
        unsafe {
            DROP_BOOTSTRAP_CONNECTIONS_CALLBACK.expect("DROP_CALLBACK missing")(self.cpp_handle)
        };
    }
}

pub trait BootstrapConnectionsExt {
    fn pool_connection(&self, client: Arc<BootstrapClient>, new_client: bool, push_front: bool);
    fn requeue_pull(&self, pull: PullInfo, network_error: bool);
}

impl BootstrapConnectionsExt for Arc<BootstrapConnections> {
    fn pool_connection(&self, client: Arc<BootstrapClient>, new_client: bool, push_front: bool) {
        todo!()
    }

    fn requeue_pull(&self, pull: PullInfo, network_error: bool) {
        todo!()
    }
}
