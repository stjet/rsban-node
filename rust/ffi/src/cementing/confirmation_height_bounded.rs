use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::BlockHash;
use rsnano_node::cementing::{truncate_after, ConfirmationHeightBounded};

use crate::{
    copy_hash_bytes,
    ledger::datastore::{TransactionHandle, WriteDatabaseQueueHandle},
    utils::TimerHandle,
};

pub struct ConfirmationHeightBoundedHandle(ConfirmationHeightBounded);

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_create(
    write_db_queue: *mut WriteDatabaseQueueHandle,
) -> *mut ConfirmationHeightBoundedHandle {
    Box::into_raw(Box::new(ConfirmationHeightBoundedHandle(
        ConfirmationHeightBounded::new((*write_db_queue).0.clone()),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_destroy(
    handle: *mut ConfirmationHeightBoundedHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_cement_blocks(
    handle: *mut ConfirmationHeightBoundedHandle,
    timer: *mut TimerHandle,
    txn: *mut TransactionHandle,
) {
    let new_timer = (*handle).0.cement_blocks((*timer).0, (*txn).as_write_txn());
    (*timer).0 = new_timer;
}

// ----------------------------------
// HashCircularBuffer:

pub struct HashCircularBufferHandle(BoundedVecDeque<BlockHash>);

#[no_mangle]
pub extern "C" fn rsn_hash_circular_buffer_create(
    max_size: usize,
) -> *mut HashCircularBufferHandle {
    Box::into_raw(Box::new(HashCircularBufferHandle(BoundedVecDeque::new(
        max_size,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hash_circular_buffer_destroy(handle: *mut HashCircularBufferHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hash_circular_buffer_push_back(
    handle: *mut HashCircularBufferHandle,
    hash: *const u8,
) {
    (*handle).0.push_back(BlockHash::from_ptr(hash));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hash_circular_buffer_empty(
    handle: *mut HashCircularBufferHandle,
) -> bool {
    (*handle).0.is_empty()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hash_circular_buffer_back(
    handle: *mut HashCircularBufferHandle,
    result: *mut u8,
) {
    let hash = (*handle).0.back().unwrap();
    copy_hash_bytes(*hash, result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hash_circular_buffer_truncate_after(
    handle: *mut HashCircularBufferHandle,
    hash: *const u8,
) {
    truncate_after(&mut (*handle).0, &BlockHash::from_ptr(hash));
}
