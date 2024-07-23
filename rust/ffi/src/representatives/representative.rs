use crate::transport::ChannelHandle;
use rsnano_core::Account;
use rsnano_node::representatives::PeeredRep;
use std::sync::Arc;

pub struct RepresentativeHandle(pub PeeredRep);

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_create(
    account: *const u8,
    channel: &ChannelHandle,
) -> *mut RepresentativeHandle {
    Box::into_raw(Box::new(RepresentativeHandle(PeeredRep::new(
        Account::from_ptr(account),
        Arc::clone(channel),
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
    handle: &RepresentativeHandle,
    account: *mut u8,
) {
    handle.0.account.copy_bytes(account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_channel(
    handle: &RepresentativeHandle,
) -> *mut ChannelHandle {
    ChannelHandle::new(Arc::clone(&handle.0.channel))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_set_channel(
    handle: &mut RepresentativeHandle,
    channel: &ChannelHandle,
) {
    handle.0.channel = Arc::clone(channel);
}
