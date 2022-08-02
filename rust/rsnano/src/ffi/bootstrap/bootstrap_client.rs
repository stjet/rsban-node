use crate::bootstrap::BootstrapClient;

pub struct BootstrapClientHandle(BootstrapClient);

#[no_mangle]
pub extern "C" fn rsn_bootstrap_client_create() -> *mut BootstrapClientHandle {
    Box::into_raw(Box::new(BootstrapClientHandle(BootstrapClient::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_destroy(handle: *mut BootstrapClientHandle) {
    drop(Box::from_raw(handle))
}
