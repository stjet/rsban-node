use num::FromPrimitive;
use rsnano_core::HashOrAccount;

use super::{
    create_message_handle3, downcast_message, downcast_message_mut, message_handle_clone,
    MessageHandle, MessageHeaderHandle,
};
use crate::{copy_hash_or_account_bytes, NetworkConstantsDto};
use rsnano_node::messages::{
    AccountInfoReqPayload, AscPullReqPayload, AscPullReqType, BlocksReqPayload, MessageEnum,
    Payload,
};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_create_accounts(
    constants: *mut NetworkConstantsDto,
    id: u64,
    target: *const u8,
    target_type: u8,
) -> *mut MessageHandle {
    let payload = AccountInfoReqPayload {
        target: HashOrAccount::from_ptr(target),
        target_type: FromPrimitive::from_u8(target_type).unwrap(),
    };
    create_message_handle3(constants, |protocol| {
        MessageEnum::new_asc_pull_req_accounts(protocol, id, payload)
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_create_blocks(
    constants: *mut NetworkConstantsDto,
    id: u64,
    start: *const u8,
    count: u8,
    start_type: u8,
) -> *mut MessageHandle {
    let payload = BlocksReqPayload {
        start: HashOrAccount::from_ptr(start),
        count,
        start_type: FromPrimitive::from_u8(start_type).unwrap(),
    };
    create_message_handle3(constants, |protocol| {
        MessageEnum::new_asc_pull_req_blocks(protocol, id, payload)
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<MessageEnum>(handle)
}

unsafe fn get_payload(handle: *mut MessageHandle) -> &'static AscPullReqPayload {
    let message = downcast_message::<MessageEnum>(handle);
    let Payload::AscPullReq(payload) = &message.payload else {panic!("not an asc_pull_req")};
    payload
}

unsafe fn get_payload_mut(handle: *mut MessageHandle) -> &'static mut AscPullReqPayload {
    let message = downcast_message_mut::<MessageEnum>(handle);
    let Payload::AscPullReq(payload) = &mut message.payload else {panic!("not an asc_pull_req")};
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_set_id(handle: *mut MessageHandle, id: u64) {
    get_payload_mut(handle).id = id;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_get_id(handle: *mut MessageHandle) -> u64 {
    get_payload(handle).id
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_pull_type(handle: *mut MessageHandle) -> u8 {
    get_payload(handle).payload_type() as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_size(header: *mut MessageHeaderHandle) -> usize {
    AscPullReqPayload::serialized_size(&*header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_payload_type(handle: *mut MessageHandle) -> u8 {
    get_payload(handle).payload_type() as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_payload_blocks(
    handle: *mut MessageHandle,
    start: *mut u8,
    count: *mut u8,
    start_type: *mut u8,
) {
    match &get_payload(handle).req_type {
        AscPullReqType::Blocks(blocks) => {
            copy_hash_or_account_bytes(blocks.start, start);
            (*count) = blocks.count;
            *start_type = blocks.start_type as u8;
        }
        _ => panic!("not a blocks payload"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_payload_account_info(
    handle: *mut MessageHandle,
    target: *mut u8,
    target_type: *mut u8,
) {
    match &get_payload(handle).req_type {
        AscPullReqType::AccountInfo(account_info) => {
            copy_hash_or_account_bytes(account_info.target, target);
            *target_type = account_info.target_type as u8;
        }
        _ => panic!("not an account_info payload"),
    }
}
