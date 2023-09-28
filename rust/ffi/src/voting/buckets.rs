use std::sync::Arc;

use crate::core::BlockHandle;
use rsnano_core::Amount;
use rsnano_node::voting::Buckets;

pub struct PrioritizationHandle(Buckets);

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_create(maximum: u64) -> *mut PrioritizationHandle {
    let info = Buckets::new(maximum as usize);
    Box::into_raw(Box::new(PrioritizationHandle(info)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_destroy(handle: *mut PrioritizationHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_size(handle: *const PrioritizationHandle) -> usize {
    (*handle).0.size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_bucket_count(
    handle: *mut PrioritizationHandle,
) -> usize {
    (*handle).0.bucket_count()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_bucket_size(
    handle: *mut PrioritizationHandle,
    index: usize,
) -> usize {
    (*handle).0.bucket_size(index)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_empty(handle: *mut PrioritizationHandle) -> bool {
    (*handle).0.empty()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_pop(handle: *mut PrioritizationHandle) {
    (*handle).0.pop()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_push(
    handle: &mut PrioritizationHandle,
    time: u64,
    block: &BlockHandle,
    priority: *const u8,
) {
    handle
        .0
        .push(time, Arc::clone(&block), Amount::from_ptr(priority))
}

#[no_mangle]
pub extern "C" fn rsn_prioritization_top(handle: &PrioritizationHandle) -> *mut BlockHandle {
    Box::into_raw(Box::new(BlockHandle(Arc::clone(handle.0.top()))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_dump(handle: *mut PrioritizationHandle) {
    (*handle).0.dump()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_index(
    handle: *mut PrioritizationHandle,
    amount: *const u8,
) -> usize {
    (*handle).0.index(&Amount::from_ptr(amount))
}
