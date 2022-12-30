use rsnano_node::confirmation_height::ConfirmationHeightUnbounded;

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
