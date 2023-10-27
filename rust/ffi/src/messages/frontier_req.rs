use rsnano_core::Account;
use rsnano_node::messages::FrontierReq;

use super::{
    create_message_handle2, create_message_handle3, downcast_message, downcast_message_mut,
    MessageHandle, MessageHeaderHandle,
};
use crate::{copy_account_bytes, NetworkConstantsDto, StringDto};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle3(constants, FrontierReq::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, FrontierReq::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_clone(
    other: *mut MessageHandle,
) -> *mut MessageHandle {
    MessageHandle::from_message(downcast_message::<FrontierReq>(other).clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_set_start(
    handle: *mut MessageHandle,
    account: *const u8,
) {
    downcast_message_mut::<FrontierReq>(handle).start = Account::from_ptr(account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_set_age(handle: *mut MessageHandle, age: u32) {
    downcast_message_mut::<FrontierReq>(handle).age = age;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_set_count(
    handle: *mut MessageHandle,
    count: u32,
) {
    downcast_message_mut::<FrontierReq>(handle).count = count;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_size() -> usize {
    FrontierReq::serialized_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_start(
    handle: *mut MessageHandle,
    account: *mut u8,
) {
    let start = downcast_message::<FrontierReq>(handle).start;
    copy_account_bytes(start, account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_age(handle: *mut MessageHandle) -> u32 {
    downcast_message::<FrontierReq>(handle).age
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_count(handle: *mut MessageHandle) -> u32 {
    downcast_message::<FrontierReq>(handle).count
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_is_confirmed_present(
    handle: *mut MessageHandle,
) -> bool {
    downcast_message::<FrontierReq>(handle).is_confirmed_present()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_to_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    (*result) = downcast_message_mut::<FrontierReq>(handle)
        .to_string()
        .into();
}
