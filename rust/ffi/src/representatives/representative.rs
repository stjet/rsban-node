use crate::transport::ChannelHandle;
use rsnano_core::Account;
use rsnano_node::representatives::Representative;

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
    handle: &RepresentativeHandle,
    account: *mut u8,
) {
    handle.0.account.copy_bytes(account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_channel(
    handle: *const RepresentativeHandle,
) -> *mut ChannelHandle {
    ChannelHandle::new((*handle).0.channel.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_set_channel(
    handle: &mut RepresentativeHandle,
    channel: &ChannelHandle,
) {
    handle.0.channel = channel.0.clone();
}
