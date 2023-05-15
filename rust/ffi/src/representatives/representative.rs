use rsnano_core::Account;
use rsnano_node::representatives::Representative;

use crate::copy_account_bytes;

pub struct RepresentativeHandle(Representative);

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_create(
    account: *const u8,
) -> *mut RepresentativeHandle {
    Box::into_raw(Box::new(RepresentativeHandle(Representative::new(
        Account::from_ptr(account),
    ))))
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

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_account(
    handle: *const RepresentativeHandle,
    account: *mut u8,
) {
    copy_account_bytes(*(*handle).0.account(), account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_last_request(
    handle: *const RepresentativeHandle,
) -> u64 {
    (*handle).0.last_request()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_set_last_request(
    handle: *mut RepresentativeHandle,
    value: u64,
) {
    (*handle).0.set_last_request(value);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_last_response(
    handle: *const RepresentativeHandle,
) -> u64 {
    (*handle).0.last_request()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_set_last_response(
    handle: *mut RepresentativeHandle,
    value: u64,
) {
    (*handle).0.set_last_response(value);
}
