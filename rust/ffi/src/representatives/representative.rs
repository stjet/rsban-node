use rsnano_node::representatives::Representative;

pub struct RepresentativeHandle(Representative);

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_create() -> *mut RepresentativeHandle {
    Box::into_raw(Box::new(RepresentativeHandle(Representative::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_destroy(handle: *mut RepresentativeHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_clone(
    handle: *mut RepresentativeHandle,
) -> *mut RepresentativeHandle {
    Box::into_raw(Box::new(RepresentativeHandle((*handle).0.clone())))
}
