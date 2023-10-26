use crate::NetworkConstantsDto;
use rsnano_node::messages::BulkPush;

use super::{create_message_handle, create_message_handle2, MessageHandle, MessageHeaderHandle};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_push_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, BulkPush::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_push_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, BulkPush::with_header)
}
