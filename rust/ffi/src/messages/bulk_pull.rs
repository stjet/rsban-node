use super::{create_message_handle2, MessageHandle};
use crate::{copy_hash_bytes, NetworkConstantsDto, StringDto};
use rsnano_core::{BlockHash, HashOrAccount};
use rsnano_node::messages::{BulkPullPayload, Payload};
use std::ops::Deref;

#[repr(C)]
pub struct BulkPullPayloadDto {
    pub start: [u8; 32],
    pub end: [u8; 32],
    pub count: u32,
    pub ascending: bool,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_create3(
    constants: *mut NetworkConstantsDto,
    dto: &BulkPullPayloadDto,
) -> *mut MessageHandle {
    create_message_handle2(constants, || {
        let payload = BulkPullPayload {
            start: HashOrAccount::from_bytes(dto.start),
            end: BlockHash::from_bytes(dto.end),
            count: dto.count,
            ascending: dto.ascending,
        };
        Payload::BulkPull(payload)
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_req_clone(
    other: &MessageHandle,
) -> *mut MessageHandle {
    MessageHandle::new(other.deref().clone())
}

unsafe fn get_payload(handle: &MessageHandle) -> &BulkPullPayload {
    let Payload::BulkPull(payload) = &handle.message else {panic!("not a bulk_pull message")};
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_end(handle: &MessageHandle, end: *mut u8) {
    copy_hash_bytes(get_payload(handle).end, end);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_to_string(
    handle: &MessageHandle,
    result: *mut StringDto,
) {
    (*result) = handle.message.to_string().into();
}
