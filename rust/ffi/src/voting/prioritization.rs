use std::cmp::Ordering;

use crate::core::BlockHandle;
use rsnano_core::Amount;
use rsnano_node::voting::{Prioritization, ValueType};

pub struct ValueTypeHandle(ValueType);

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_create_value_type(
    time: u64,
    block: *const BlockHandle,
) -> *mut ValueTypeHandle {
    let block = (*block).block.clone();
    let info = ValueType::new(time, block);
    Box::into_raw(Box::new(ValueTypeHandle(info)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_drop_value_type(handle: *mut ValueTypeHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_value_type_cmp(
    first: *mut ValueTypeHandle,
    second: *mut ValueTypeHandle,
) -> i32 {
    match (*first).0.cmp(&(*second).0) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_get_value_type_time(
    handle: *const ValueTypeHandle,
) -> u64 {
    (*handle).0.time
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_get_value_type_block(
    handle: *const ValueTypeHandle,
) -> *mut BlockHandle {
    Box::into_raw(Box::new(BlockHandle::new((*handle).0.block.clone())))
}

pub struct PrioritizationHandle(Prioritization);

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_create(maximum: u64) -> *mut PrioritizationHandle {
    let info = Prioritization::new(maximum);
    Box::into_raw(Box::new(PrioritizationHandle(info)))
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
    handle: *mut PrioritizationHandle,
    time: u64,
    block: *const BlockHandle,
    priority: *const u8,
) {
    (*handle)
        .0
        .push(time, (*block).block.clone(), Amount::from_ptr(priority))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_prioritization_top(
    handle: *mut PrioritizationHandle,
) -> *mut BlockHandle {
    Box::into_raw(Box::new(BlockHandle::new((*handle).0.top().clone())))
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
