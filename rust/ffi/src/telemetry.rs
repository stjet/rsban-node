use crate::{
    block_processing::UncheckedMapHandle,
    ledger::datastore::LedgerHandle,
    messages::{MessageHandle, TelemetryDataHandle},
    transport::{ChannelHandle, EndpointDto, TcpChannelsHandle},
    utils::{ContainerInfoComponentHandle, ContextWrapper},
    NetworkParamsDto, NodeConfigDto, StatHandle, VoidPointerCallback,
};
use rsnano_core::KeyPair;
use rsnano_messages::{Message, TelemetryData};
use rsnano_node::{
    config::NodeConfig, consolidate_telemetry_data, transport::ChannelEnum, NetworkParams,
    TelementryConfig, TelementryExt, Telemetry,
};
use std::{
    ffi::{c_char, c_void, CStr},
    net::SocketAddrV6,
    ops::Deref,
    sync::Arc,
};

pub struct TelemetryHandle(pub Arc<Telemetry>);

impl Deref for TelemetryHandle {
    type Target = Arc<Telemetry>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type TelemetryNotifyCallback =
    extern "C" fn(*mut c_void, *mut TelemetryDataHandle, *mut ChannelHandle);

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_destroy(handle: *mut TelemetryHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_local_telemetry(
    handle: &TelemetryHandle,
) -> *mut TelemetryDataHandle {
    TelemetryDataHandle::new(handle.0.local_telemetry())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_start(handle: &TelemetryHandle) {
    handle.0.start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_stop(handle: &TelemetryHandle) {
    handle.0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_process(
    handle: &TelemetryHandle,
    message: &MessageHandle,
    channel: &ChannelHandle,
) {
    let Message::TelemetryAck(ack) = &message.0.message else {
        panic!("not a TelemetryAck")
    };
    handle.0.process(ack, channel);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_trigger(handle: &TelemetryHandle) {
    handle.0.trigger();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_len(handle: &TelemetryHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_get_telemetry(
    handle: &TelemetryHandle,
    endpoint: &EndpointDto,
) -> *mut TelemetryDataHandle {
    match handle.0.get_telemetry(&endpoint.into()) {
        Some(data) => TelemetryDataHandle::new(data),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_get_all(
    handle: &TelemetryHandle,
) -> *mut TelemetryDataMapHandle {
    Box::into_raw(Box::new(TelemetryDataMapHandle(
        handle.0.get_all_telemetries().drain().collect(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_collect_container_info(
    handle: &TelemetryHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = handle
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}

pub struct TelemetryDataMapHandle(Vec<(SocketAddrV6, TelemetryData)>);

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_map_destroy(handle: *mut TelemetryDataMapHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_telemetry_data_map_len(handle: &TelemetryDataMapHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub extern "C" fn rsn_telemetry_data_map_get(
    handle: &TelemetryDataMapHandle,
    index: usize,
    endpoint: &mut EndpointDto,
) -> *mut TelemetryDataHandle {
    let (ep, data) = &handle.0[index];
    *endpoint = ep.into();
    TelemetryDataHandle::new(data.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_consolidate_telemetry_data(
    datas: *const *const TelemetryDataHandle,
    len: usize,
) -> *mut TelemetryDataHandle {
    let datas: Vec<_> = if datas.is_null() {
        &[]
    } else {
        std::slice::from_raw_parts(datas, len)
    }
    .iter()
    .map(|i| (**i).clone())
    .collect();

    TelemetryDataHandle::new(consolidate_telemetry_data(&datas))
}
