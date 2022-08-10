use crate::bootstrap::BootstrapAttempts;
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use super::bootstrap_attempt::BootstrapAttemptHandle;

pub struct BootstrapAttemptsHandle(Mutex<BootstrapAttempts>);

#[no_mangle]
pub extern "C" fn rsn_bootstrap_attempts_create() -> *mut BootstrapAttemptsHandle {
    Box::into_raw(Box::new(BootstrapAttemptsHandle(Mutex::new(
        BootstrapAttempts::new(),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempts_destroy(handle: *mut BootstrapAttemptsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempts_add(
    handle: *mut BootstrapAttemptsHandle,
    attempt: *mut BootstrapAttemptHandle,
) {
    (*handle)
        .0
        .lock()
        .unwrap()
        .add(Arc::clone((*attempt).deref()))
}
