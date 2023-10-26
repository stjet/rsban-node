use super::{
    create_message_handle, create_message_handle2, downcast_message_mut, message_handle_clone,
    MessageHandle, MessageHeaderHandle,
};
use crate::{NetworkConstantsDto, StringDto};
use rsnano_node::messages::TelemetryReq;

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, TelemetryReq::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, TelemetryReq::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<TelemetryReq>(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_to_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    (*result) = downcast_message_mut::<TelemetryReq>(handle)
        .to_string()
        .into();
}
