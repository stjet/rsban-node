use rsnano_node::bootstrap::BootstrapAttempts;
use std::sync::Mutex;
pub struct BootstrapAttemptsHandle {
    _data: Mutex<BootstrapAttempts>,
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_attempts_create() -> *mut BootstrapAttemptsHandle {
    Box::into_raw(Box::new(BootstrapAttemptsHandle {
        _data: Mutex::new(BootstrapAttempts::new()),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempts_destroy(handle: *mut BootstrapAttemptsHandle) {
    drop(Box::from_raw(handle))
}
