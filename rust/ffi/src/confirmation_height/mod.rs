mod conf_height_details;

use rsnano_node::confirmation_height::ConfirmationHeightUnbounded;

use self::conf_height_details::ConfHeightDetailsHandle;

pub struct ConfirmationHeightUnboundedHandle(ConfirmationHeightUnbounded);

#[no_mangle]
pub extern "C" fn rsn_conf_height_unbounded_create() -> *mut ConfirmationHeightUnboundedHandle {
    Box::into_raw(Box::new(ConfirmationHeightUnboundedHandle(
        ConfirmationHeightUnbounded::new(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_destroy(
    handle: *mut ConfirmationHeightUnboundedHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_add(
    handle: *mut ConfirmationHeightUnboundedHandle,
    details: *const ConfHeightDetailsHandle,
) {
    (*handle).0.pending_writes.push_back((*details).0.clone());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_erase_first(
    handle: *mut ConfirmationHeightUnboundedHandle,
) {
    (*handle).0.pending_writes.pop_front();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_size(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> usize {
    (*handle).0.pending_writes.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_front(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> *mut ConfHeightDetailsHandle {
    Box::into_raw(Box::new(ConfHeightDetailsHandle(
        (*handle).0.pending_writes.front().unwrap().clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_total_pending_write_block_count(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> u64 {
    (*handle).0.total_pending_write_block_count()
}
