use super::{create_message_handle2, MessageHandle};
use crate::{NetworkConstantsDto, StringDto};
use rsnano_core::Account;
use rsnano_messages::{FrontierReq, Message};

#[repr(C)]
pub struct FrontierReqPayloadDto {
    pub start: [u8; 32],
    pub age: u32,
    pub count: u32,
    pub only_confirmed: bool,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_create3(
    constants: *mut NetworkConstantsDto,
    payload: &FrontierReqPayloadDto,
) -> *mut MessageHandle {
    create_message_handle2(constants, || {
        Message::FrontierReq(FrontierReq {
            start: Account::from_bytes(payload.start),
            age: payload.age,
            count: payload.count,
            only_confirmed: payload.only_confirmed,
        })
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_clone(
    other: &MessageHandle,
) -> *mut MessageHandle {
    MessageHandle::new(other.0.clone())
}

unsafe fn get_payload(handle: &MessageHandle) -> &FrontierReq {
    let Message::FrontierReq(payload) = &handle.message else {
        panic!("not a frontier_req")
    };
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_size() -> usize {
    FrontierReq::serialized_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_start(handle: &MessageHandle, account: *mut u8) {
    let start = get_payload(handle).start;
    start.copy_bytes(account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_age(handle: &MessageHandle) -> u32 {
    get_payload(handle).age
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_count(handle: &mut MessageHandle) -> u32 {
    get_payload(handle).count
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_is_confirmed_present(
    handle: &mut MessageHandle,
) -> bool {
    get_payload(handle).only_confirmed
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_to_string(
    handle: &MessageHandle,
    result: *mut StringDto,
) {
    (*result) = handle.message.to_string().into();
}
