use crate::messages::TelemetryData;

pub struct TelemetryDataHandle(TelemetryData);

#[no_mangle]
pub extern "C" fn rsn_telemetry_data_create() -> *mut TelemetryDataHandle {
    Box::into_raw(Box::new(TelemetryDataHandle(TelemetryData::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_destroy(handle: *mut TelemetryDataHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_clone(
    handle: *mut TelemetryDataHandle,
) -> *mut TelemetryDataHandle {
    Box::into_raw(Box::new(TelemetryDataHandle((*handle).0.clone())))
}
