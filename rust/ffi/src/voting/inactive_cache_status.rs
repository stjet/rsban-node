use crate::{copy_amount_bytes, StringDto};
use rsnano_core::Amount;
use rsnano_node::voting::InactiveCacheStatus;

pub struct InactiveCacheStatusHandle(pub(crate) InactiveCacheStatus);

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_create() -> *mut InactiveCacheStatusHandle {
    let info = InactiveCacheStatus::default();
    Box::into_raw(Box::new(InactiveCacheStatusHandle(info)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_destroy(handle: *mut InactiveCacheStatusHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_bootstrap_started(
    handle: *const InactiveCacheStatusHandle,
) -> bool {
    (*handle).0.bootstrap_started
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_election_started(
    handle: *const InactiveCacheStatusHandle,
) -> bool {
    (*handle).0.election_started
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_confirmed(
    handle: *const InactiveCacheStatusHandle,
) -> bool {
    (*handle).0.confirmed
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_tally(
    handle: *const InactiveCacheStatusHandle,
    result: *mut u8,
) {
    copy_amount_bytes((*handle).0.tally, result)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_set_bootstrap_started(
    handle: *mut InactiveCacheStatusHandle,
    bootstrap_started: bool,
) {
    (*handle).0.bootstrap_started = bootstrap_started;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_set_election_started(
    handle: *mut InactiveCacheStatusHandle,
    election_started: bool,
) {
    (*handle).0.election_started = election_started;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_set_confirmed(
    handle: *mut InactiveCacheStatusHandle,
    confirmed: bool,
) {
    (*handle).0.confirmed = confirmed;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_set_tally(
    handle: *mut InactiveCacheStatusHandle,
    tally: *const u8,
) {
    (*handle).0.tally = Amount::from_ptr(tally)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_eq(
    first: *const InactiveCacheStatusHandle,
    second: *const InactiveCacheStatusHandle,
) -> bool {
    (*first).0.eq(&(*second).0)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_status_to_string(
    handle: *const InactiveCacheStatusHandle,
    result: *mut StringDto,
) {
    (*result) = (*handle).0.to_string().into();
}
