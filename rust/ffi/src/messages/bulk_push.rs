use crate::NetworkConstantsDto;
use rsnano_node::messages::{MessageEnum, Payload};

use super::{create_message_handle2, create_message_handle3, MessageHandle, MessageHeaderHandle};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_push_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle3(constants, MessageEnum::new_bulk_push)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_push_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, |header| MessageEnum {
        header,
        payload: Payload::BulkPush,
    })
}
