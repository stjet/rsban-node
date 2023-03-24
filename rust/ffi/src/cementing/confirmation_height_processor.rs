use std::sync::{Arc, Mutex, MutexGuard};

use rsnano_node::cementing::{ConfirmationHeightProcessor, GuardedData};

pub struct ConfirmationHeightProcessorHandle(ConfirmationHeightProcessor);

#[no_mangle]
pub extern "C" fn rsn_confirmation_height_processor_create(
) -> *mut ConfirmationHeightProcessorHandle {
    Box::into_raw(Box::new(ConfirmationHeightProcessorHandle(
        ConfirmationHeightProcessor::new(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_destroy(
    handle: *mut ConfirmationHeightProcessorHandle,
) {
    drop(Box::from_raw(handle))
}

//----------------------------------------
// Mutex
//----------------------------------------

pub struct ConfirmationHeightProcessorMutex(Arc<Mutex<GuardedData>>);
pub struct ConfirmationHeightProcessorLock {
    mutex: Arc<Mutex<GuardedData>>,
    guard: Option<MutexGuard<'static, GuardedData>>,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_get_mutex(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> *mut ConfirmationHeightProcessorMutex {
    Box::into_raw(Box::new(ConfirmationHeightProcessorMutex(
        (*handle).0.guarded_data.clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_mutex_destroy(
    handle: *mut ConfirmationHeightProcessorMutex,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_mutex_lock(
    handle: *mut ConfirmationHeightProcessorMutex,
) -> *mut ConfirmationHeightProcessorLock {
    let guard = (*handle).0.lock().unwrap();
    let guard =
        std::mem::transmute::<MutexGuard<GuardedData>, MutexGuard<'static, GuardedData>>(guard);
    Box::into_raw(Box::new(ConfirmationHeightProcessorLock {
        mutex: (*handle).0.clone(),
        guard: Some(guard),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_lock_destroy(
    handle: *mut ConfirmationHeightProcessorLock,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_lock_unlock(
    handle: *mut ConfirmationHeightProcessorLock,
) {
    drop((*handle).guard.take());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_lock_relock(
    handle: *mut ConfirmationHeightProcessorLock,
) {
    drop((*handle).guard.take());
    let guard = (*handle).mutex.lock().unwrap();
    let guard =
        std::mem::transmute::<MutexGuard<GuardedData>, MutexGuard<'static, GuardedData>>(guard);
    (*handle).guard = Some(guard);
}
