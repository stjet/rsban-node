use super::{create_message_handle2, MessageHandle};
use crate::NetworkConstantsDto;
use rsnano_node::messages::Message;

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_push_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle2(constants, || Message::BulkPush)
}
