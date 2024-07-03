use crate::FfiPropertyTree;
use rsnano_node::bootstrap::BootstrapAttempts;
use std::{
    ffi::c_void,
    ops::Deref,
    sync::{Arc, Mutex},
};

pub struct BootstrapAttemptsHandle(Arc<Mutex<BootstrapAttempts>>);

impl BootstrapAttemptsHandle {
    pub fn new(attempts: Arc<Mutex<BootstrapAttempts>>) -> *mut Self {
        Box::into_raw(Box::new(Self(attempts)))
    }
}

impl Deref for BootstrapAttemptsHandle {
    type Target = Arc<Mutex<BootstrapAttempts>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_attempts_create() -> *mut BootstrapAttemptsHandle {
    Box::into_raw(Box::new(BootstrapAttemptsHandle(Arc::new(Mutex::new(
        BootstrapAttempts::new(),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempts_destroy(handle: *mut BootstrapAttemptsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_attempts_size(handle: &BootstrapAttemptsHandle) -> usize {
    handle.lock().unwrap().size()
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_attempts_total_attempts(handle: &BootstrapAttemptsHandle) -> usize {
    handle.lock().unwrap().total_attempts()
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_attempts_attempts_information(
    handle: &BootstrapAttemptsHandle,
    tree: *mut c_void,
) {
    handle
        .lock()
        .unwrap()
        .attempts_information(&mut FfiPropertyTree::new_borrowed(tree));
}
