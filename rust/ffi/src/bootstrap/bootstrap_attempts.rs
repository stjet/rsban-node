use super::bootstrap_attempt::BootstrapAttemptHandle;
use crate::FfiPropertyTree;
use rsnano_node::bootstrap::BootstrapAttempts;
use std::{
    ffi::c_void,
    ops::Deref,
    sync::{Arc, Mutex},
};

pub struct BootstrapAttemptsHandle(Arc<Mutex<BootstrapAttempts>>);

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
pub extern "C" fn rsn_bootstrap_attempts_incremental_id(handle: &BootstrapAttemptsHandle) -> usize {
    handle.lock().unwrap().get_incremental_id()
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_attempts_add(
    handle: &BootstrapAttemptsHandle,
    attempt: &BootstrapAttemptHandle,
) {
    handle.lock().unwrap().add(Arc::clone(attempt))
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_attempts_remove(
    handle: &BootstrapAttemptsHandle,
    incremental_id: usize,
) {
    handle.lock().unwrap().remove(incremental_id);
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_attempts_clear(handle: &BootstrapAttemptsHandle) {
    handle.lock().unwrap().clear();
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
pub unsafe extern "C" fn rsn_bootstrap_attempts_find(
    handle: &BootstrapAttemptsHandle,
    incremental_id: usize,
) -> *mut BootstrapAttemptHandle {
    let guard = handle.lock().unwrap();
    match guard.find(incremental_id) {
        Some(attempt) => BootstrapAttemptHandle::new(Arc::clone(attempt)),
        None => std::ptr::null_mut(),
    }
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
