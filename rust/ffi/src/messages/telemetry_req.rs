use super::{create_message_handle3, downcast_message_mut, message_handle_clone, MessageHandle};
use crate::{NetworkConstantsDto, StringDto};
use rsnano_node::messages::MessageEnum;

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle3(constants, MessageEnum::new_telemetry_req)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<MessageEnum>(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_to_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    (*result) = downcast_message_mut::<MessageEnum>(handle)
        .to_string()
        .into();
}
