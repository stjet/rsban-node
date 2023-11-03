use rsnano_core::Account;
use rsnano_node::messages::{FrontierReqPayload, MessageEnum, Payload};

use super::{create_message_handle3, downcast_message, downcast_message_mut, MessageHandle};
use crate::{copy_account_bytes, NetworkConstantsDto, StringDto};

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
    create_message_handle3(constants, |protocol| {
        MessageEnum::new_frontier_req(
            protocol,
            FrontierReqPayload {
                start: Account::from_bytes(payload.start),
                age: payload.age,
                count: payload.count,
                only_confirmed: payload.only_confirmed,
            },
        )
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_clone(
    other: *mut MessageHandle,
) -> *mut MessageHandle {
    MessageHandle::from_message(downcast_message::<MessageEnum>(other).clone())
}

unsafe fn get_payload(handle: *mut MessageHandle) -> &'static FrontierReqPayload {
    let msg = downcast_message::<MessageEnum>(handle);
    let Payload::FrontierReq(payload) = &msg.payload else { panic!("not a frontier_req")};
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_size() -> usize {
    FrontierReqPayload::serialized_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_start(
    handle: *mut MessageHandle,
    account: *mut u8,
) {
    let start = get_payload(handle).start;
    copy_account_bytes(start, account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_age(handle: *mut MessageHandle) -> u32 {
    get_payload(handle).age
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_count(handle: *mut MessageHandle) -> u32 {
    get_payload(handle).count
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_is_confirmed_present(
    handle: *mut MessageHandle,
) -> bool {
    get_payload(handle).only_confirmed
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_to_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    (*result) = downcast_message_mut::<MessageEnum>(handle)
        .to_string()
        .into();
}
