use rsnano_core::{
    utils::{system_time_as_nanoseconds, system_time_from_nanoseconds},
    Account,
};

use rsnano_node::representatives::Representative;

use crate::{copy_account_bytes, transport::ChannelHandle};

pub struct RepresentativeHandle(pub Representative);

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_create(
    account: *const u8,
    channel: *mut ChannelHandle,
) -> *mut RepresentativeHandle {
    Box::into_raw(Box::new(RepresentativeHandle(Representative::new(
        Account::from_ptr(account),
        (*channel).0.clone(),
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
    system_time_as_nanoseconds((*handle).0.last_request())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_set_last_request(
    handle: *mut RepresentativeHandle,
    value: u64,
) {
    (*handle)
        .0
        .set_last_request(system_time_from_nanoseconds(value));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_last_response(
    handle: *const RepresentativeHandle,
) -> u64 {
    system_time_as_nanoseconds((*handle).0.last_request())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_set_last_response(
    handle: *mut RepresentativeHandle,
    value: u64,
) {
    (*handle)
        .0
        .set_last_response(system_time_from_nanoseconds(value));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_channel(
    handle: *const RepresentativeHandle,
) -> *mut ChannelHandle {
    ChannelHandle::new((*handle).0.channel().clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_set_channel(
    handle: *mut RepresentativeHandle,
    channel: *const ChannelHandle,
) {
    (*handle).0.set_channel((*channel).0.clone());
}
