use std::collections::VecDeque;

use rsnano_core::BlockHash;
use rsnano_node::cementing::ConfirmationHeightBounded;

pub struct ConfirmationHeightBoundedHandle(ConfirmationHeightBounded);

#[no_mangle]
pub extern "C" fn rsn_confirmation_height_bounded_create() -> *mut ConfirmationHeightBoundedHandle {
    Box::into_raw(Box::new(ConfirmationHeightBoundedHandle(
        ConfirmationHeightBounded::new(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_destroy(
    handle: *mut ConfirmationHeightBoundedHandle,
) {
    drop(Box::from_raw(handle))
}

pub struct HashCircularBufferHandle(VecDeque<BlockHash>);

#[no_mangle]
pub extern "C" fn rsn_hash_circular_buffer_create() -> *mut HashCircularBufferHandle {
    Box::into_raw(Box::new(HashCircularBufferHandle(VecDeque::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hash_circular_buffer_destroy(handle: *mut HashCircularBufferHandle) {
    drop(Box::from_raw(handle))
}
