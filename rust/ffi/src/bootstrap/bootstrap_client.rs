use crate::transport::SocketHandle;
use rsnano_node::bootstrap::BootstrapClient;
use std::{ops::Deref, sync::Arc};

pub struct BootstrapClientHandle(pub Arc<BootstrapClient>);

impl Deref for BootstrapClientHandle {
    type Target = Arc<BootstrapClient>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_destroy(handle: *mut BootstrapClientHandle) {
    drop(Box::from_raw(handle))
}
