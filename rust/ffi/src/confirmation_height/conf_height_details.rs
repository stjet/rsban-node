use rsnano_node::confirmation_height::ConfHeightDetails;

pub struct ConfHeightDetailsHandle(ConfHeightDetails);

#[no_mangle]
pub extern "C" fn rsn_conf_height_details_create() -> *mut ConfHeightDetailsHandle {
    Box::into_raw(Box::new(ConfHeightDetailsHandle(ConfHeightDetails::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_clone(
    handle: *const ConfHeightDetailsHandle,
) -> *mut ConfHeightDetailsHandle {
    Box::into_raw(Box::new(ConfHeightDetailsHandle((*handle).0.clone())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_destroy(handle: *mut ConfHeightDetailsHandle) {
    drop(Box::from_raw(handle))
}
