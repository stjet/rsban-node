use rsnano_core::{BlockHash, HashOrAccount};

use crate::{copy_hash_bytes, copy_hash_or_account_bytes, NetworkConstantsDto, StringDto};
use rsnano_node::messages::{BulkPull, BulkPullPayload};

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
        BulkPull::new_bulk_pull(protocol, payload)
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_req_clone(
    other: *mut MessageHandle,
) -> *mut MessageHandle {
    MessageHandle::from_message(downcast_message::<BulkPull>(other).clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_end(handle: *mut MessageHandle, end: *mut u8) {
    copy_hash_bytes(downcast_message::<BulkPull>(handle).payload.end, end);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_to_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    (*result) = downcast_message_mut::<BulkPull>(handle).to_string().into();
}
