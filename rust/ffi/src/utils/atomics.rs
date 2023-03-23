use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

//------------------------------------------------------------------------------
// Atomic U64
//------------------------------------------------------------------------------

pub struct AtomicU64Handle(pub Arc<AtomicU64>);

#[no_mangle]
pub extern "C" fn rsn_atomic_u64_create(value: u64) -> *mut AtomicU64Handle {
    Box::into_raw(Box::new(AtomicU64Handle(Arc::new(AtomicU64::new(value)))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_atomic_u64_destroy(handle: *mut AtomicU64Handle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_atomic_u64_load(handle: *mut AtomicU64Handle) -> u64 {
    (*handle).0.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_atomic_u64_store(handle: *mut AtomicU64Handle, value: u64) {
    (*handle).0.store(value, Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_atomic_u64_add(handle: *mut AtomicU64Handle, value: u64) {
    (*handle).0.fetch_add(value, Ordering::SeqCst);
}

//------------------------------------------------------------------------------
// Atomic Bool
//------------------------------------------------------------------------------

pub struct AtomicBoolHandle(pub Arc<AtomicBool>);

#[no_mangle]
pub extern "C" fn rsn_atomic_bool_create(value: bool) -> *mut AtomicBoolHandle {
    Box::into_raw(Box::new(AtomicBoolHandle(Arc::new(AtomicBool::new(value)))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_atomic_bool_destroy(handle: *mut AtomicBoolHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_atomic_bool_load(handle: *mut AtomicBoolHandle) -> bool {
    (*handle).0.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_atomic_bool_store(handle: *mut AtomicBoolHandle, value: bool) {
    (*handle).0.store(value, Ordering::SeqCst)
}
