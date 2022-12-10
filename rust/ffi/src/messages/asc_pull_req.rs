use num::FromPrimitive;
use rsnano_core::HashOrAccount;

use super::{
    create_message_handle, create_message_handle2, downcast_message, downcast_message_mut,
    message_handle_clone, MessageHandle, MessageHeaderHandle,
};
use crate::{copy_hash_or_account_bytes, utils::FfiStream, NetworkConstantsDto};
use rsnano_node::messages::{
    AccountInfoReqPayload, AscPullReq, AscPullReqPayload, BlocksReqPayload, Message,
};
use std::ffi::c_void;

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, AscPullReq::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, AscPullReq::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<AscPullReq>(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_set_id(handle: *mut MessageHandle, id: u64) {
    downcast_message_mut::<AscPullReq>(handle).id = id;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_get_id(handle: *mut MessageHandle) -> u64 {
    downcast_message::<AscPullReq>(handle).id
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_pull_type(handle: *mut MessageHandle) -> u8 {
    downcast_message::<AscPullReq>(handle).payload_type() as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_size(header: *mut MessageHeaderHandle) -> usize {
    AscPullReq::serialized_size(&*header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_deserialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    downcast_message_mut::<AscPullReq>(handle)
        .deserialize(&mut stream)
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_serialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    downcast_message::<AscPullReq>(handle)
        .serialize(&mut stream)
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_payload_type(handle: *mut MessageHandle) -> u8 {
    downcast_message::<AscPullReq>(handle).payload_type() as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_payload_blocks(
    handle: *mut MessageHandle,
    start: *mut u8,
    count: *mut u8,
    start_type: *mut u8,
) {
    match downcast_message::<AscPullReq>(handle).payload() {
        AscPullReqPayload::Blocks(blocks) => {
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
    match downcast_message::<AscPullReq>(handle).payload() {
        AscPullReqPayload::AccountInfo(account_info) => {
            copy_hash_or_account_bytes(account_info.target, target);
            *target_type = account_info.target_type as u8;
        }
        _ => panic!("not an account_info payload"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_request_blocks(
    handle: *mut MessageHandle,
    start: *const u8,
    count: u8,
    start_type: u8,
) {
    let payload = BlocksReqPayload {
        start: HashOrAccount::from_ptr(start),
        count,
        start_type: FromPrimitive::from_u8(start_type).unwrap(),
    };
    downcast_message_mut::<AscPullReq>(handle)
        .request_blocks(payload)
        .unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_request_account_info(
    handle: *mut MessageHandle,
    target: *const u8,
    target_type: u8,
) {
    let payload = AccountInfoReqPayload {
        target: HashOrAccount::from_ptr(target),
        target_type: FromPrimitive::from_u8(target_type).unwrap(),
    };
    downcast_message_mut::<AscPullReq>(handle)
        .request_account_info(payload)
        .unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_req_request_invalid(handle: *mut MessageHandle) {
    downcast_message_mut::<AscPullReq>(handle).request_invalid();
}
