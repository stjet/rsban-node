use super::{create_message_handle3, MessageHandle};
use crate::NetworkConstantsDto;
use rsnano_node::messages::MessageEnum;

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_push_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle3(constants, MessageEnum::new_bulk_push)
}
