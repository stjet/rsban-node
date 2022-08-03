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
pub unsafe extern "C" fn rsn_bootstrap_attempts_get_incremental_id(
    handle: *mut BootstrapAttemptsHandle,
) -> usize {
    (*handle).0.lock().unwrap().get_incremental_id()
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

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempts_remove(
    handle: *mut BootstrapAttemptsHandle,
    incremental_id: usize,
) {
    (*handle).0.lock().unwrap().remove(incremental_id);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempts_clear(handle: *mut BootstrapAttemptsHandle) {
    (*handle).0.lock().unwrap().clear();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempts_find(
    handle: *mut BootstrapAttemptsHandle,
    incremental_id: usize,
) -> *mut BootstrapAttemptHandle {
    match (*handle).0.lock().unwrap().find(incremental_id) {
        Some(attempt) => BootstrapAttemptHandle::new(Arc::clone(attempt)),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempts_size(
    handle: *mut BootstrapAttemptsHandle,
) -> usize {
    (*handle).0.lock().unwrap().size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempts_total_attempts(
    handle: *mut BootstrapAttemptsHandle,
) -> usize {
    (*handle).0.lock().unwrap().total_attempts()
}

#[repr(C)]
pub struct BootstrapAttemptResultDto {
    pub id: u64,
    pub attempt: *mut BootstrapAttemptHandle,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempts_attempts(
    handle: *mut BootstrapAttemptsHandle,
    result: *mut BootstrapAttemptResultDto,
    result_size: usize,
) -> usize {
    let result = std::slice::from_raw_parts_mut(result, result_size);
    let lock = (*handle).0.lock().unwrap();
    let mut count = 0;
    for (i, (&id, attempt)) in lock.attempts().iter().take(result_size).enumerate() {
        result[i].id = id as u64;
        result[i].attempt = BootstrapAttemptHandle::new(Arc::clone(attempt));
        count += 1;
    }
    count
}
