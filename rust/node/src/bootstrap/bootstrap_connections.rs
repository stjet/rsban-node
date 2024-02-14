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
pub static mut POOL_CONNECTION_CALLBACK: Option<fn(*mut c_void, Arc<BootstrapClient>, bool, bool)> =
    None;
pub static mut REQUEUE_PULL_CALLBACK: Option<fn(*mut c_void, PullInfo, bool)> = None;

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
        unsafe {
            POOL_CONNECTION_CALLBACK.expect("POOL_CONNECTION_CALLBACK missing")(
                self.cpp_handle,
                client,
                new_client,
                push_front,
            );
        }
    }

    fn requeue_pull(&self, pull: PullInfo, network_error: bool) {
        unsafe {
            REQUEUE_PULL_CALLBACK.expect("REQUEUE_PULL_CALLBACK missing")(
                self.cpp_handle,
                pull,
                network_error,
            );
        }
    }
}
