use rsnano_core::{BlockHash, HashOrAccount};

use crate::{copy_hash_bytes, NetworkConstantsDto, StringDto};
use rsnano_node::messages::{BulkPullPayload, MessageEnum, Payload};

use super::{create_message_handle3, downcast_message, downcast_message_mut, MessageHandle};

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
    create_message_handle3(constants, |protocol| {
        let payload = BulkPullPayload {
            start: HashOrAccount::from_bytes(dto.start),
            end: BlockHash::from_bytes(dto.end),
            count: dto.count,
            ascending: dto.ascending,
        };
        MessageEnum::new_bulk_pull(protocol, payload)
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_req_clone(
    other: *mut MessageHandle,
) -> *mut MessageHandle {
    MessageHandle::from_message(downcast_message::<MessageEnum>(other).clone())
}

unsafe fn get_payload(handle: *mut MessageHandle) -> &'static BulkPullPayload {
    let message = downcast_message::<MessageEnum>(handle);
    let Payload::BulkPull(payload) = &message.payload else {panic!("not a bulk_pull message")};
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_end(handle: *mut MessageHandle, end: *mut u8) {
    copy_hash_bytes(get_payload(handle).end, end);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_to_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    (*result) = downcast_message_mut::<MessageEnum>(handle)
        .to_string()
        .into();
}
