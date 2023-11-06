use super::{create_message_handle2, message_handle_clone, MessageHandle};
use crate::{NetworkConstantsDto, StringDto};
use rsnano_node::messages::{Message, TelemetryReqPayload};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle2(constants, || Message::TelemetryReq(TelemetryReqPayload {}))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_clone(
    handle: &MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_to_string(
    handle: &MessageHandle,
    result: *mut StringDto,
) {
    (*result) = handle.message.to_string().into();
}
