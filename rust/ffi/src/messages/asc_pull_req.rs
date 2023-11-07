use num::FromPrimitive;
use rsnano_core::HashOrAccount;

use super::{create_message_handle2, message_handle_clone, MessageHandle};
use crate::{copy_hash_or_account_bytes, NetworkConstantsDto};
use rsnano_node::messages::{
    AccountInfoReqPayload, AscPullReq, AscPullReqType, BlocksReqPayload, Message,
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
    create_message_handle2(constants, || {
        Message::AscPullReq(AscPullReq {
            req_type: AscPullReqType::AccountInfo(payload),
            id,
        })
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
    create_message_handle2(constants, || {
        Message::AscPullReq(AscPullReq {
            req_type: AscPullReqType::Blocks(payload),
            id,
        })
    })
}

#[no_mangle]
pub extern "C" fn rsn_message_asc_pull_req_clone(handle: &MessageHandle) -> *mut MessageHandle {
    message_handle_clone(handle)
}

fn get_payload(handle: &MessageHandle) -> &AscPullReq {
    let Message::AscPullReq(payload) = &handle.message else {
        panic!("not an asc_pull_req")
    };
    payload
}

fn get_payload_mut(handle: &mut MessageHandle) -> &mut AscPullReq {
    let Message::AscPullReq(payload) = &mut handle.message else {
        panic!("not an asc_pull_req")
    };
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_set_id(handle: &mut MessageHandle, id: u64) {
    get_payload_mut(handle).id = id;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_get_id(handle: &MessageHandle) -> u64 {
    get_payload(handle).id
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_pull_type(handle: &MessageHandle) -> u8 {
    get_payload(handle).payload_type() as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_payload_type(handle: &MessageHandle) -> u8 {
    get_payload(handle).payload_type() as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_payload_blocks(
    handle: &MessageHandle,
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
    handle: &MessageHandle,
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
