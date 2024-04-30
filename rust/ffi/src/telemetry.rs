use std::{ffi::c_void, sync::Arc};

use rsnano_core::KeyPair;
use rsnano_messages::TelemetryData;
use rsnano_node::{
    config::NodeConfig, transport::ChannelEnum, NetworkParams, TelementryConfig, Telemetry,
};

use crate::{
    block_processing::UncheckedMapHandle,
    ledger::datastore::LedgerHandle,
    messages::TelemetryDataHandle,
    transport::{ChannelHandle, TcpChannelsHandle},
    utils::ContextWrapper,
    NetworkParamsDto, NodeConfigDto, StatHandle, VoidPointerCallback,
};

pub struct TelemetryHandle(Arc<Telemetry>);

pub type TelemetryNotifyCallback =
    extern "C" fn(*mut c_void, *mut TelemetryDataHandle, *mut ChannelHandle);

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_create(
    enable_ongoing_requests: bool,
    enable_ongoing_broadcasts: bool,
    node_config: &NodeConfigDto,
    stats: &StatHandle,
    ledger: &LedgerHandle,
    unchecked: &UncheckedMapHandle,
    network_params: &NetworkParamsDto,
    channels: &TcpChannelsHandle,
    node_id: *const u8,
    notify_callback: TelemetryNotifyCallback,
    callback_context: *mut c_void,
    delete_context: VoidPointerCallback,
) -> *mut TelemetryHandle {
    let node_config = NodeConfig::try_from(node_config).unwrap();
    let config = TelementryConfig {
        enable_ongoing_requests,
        enable_ongoing_broadcasts,
    };
    let network_params = NetworkParams::try_from(network_params).unwrap();
    let node_id = KeyPair::from_priv_key_bytes(std::slice::from_raw_parts(node_id, 32)).unwrap();
    let context_wrapper = ContextWrapper::new(callback_context, delete_context);
    let notify = Box::new(move |data: &TelemetryData, channel: &Arc<ChannelEnum>| {
        let data_handle = TelemetryDataHandle::new(data.clone());
        let channel_handle = ChannelHandle::new(Arc::clone(channel));
        notify_callback(context_wrapper.get_context(), data_handle, channel_handle);
    });

    Box::into_raw(Box::new(TelemetryHandle(Arc::new(Telemetry::new(
        config,
        node_config,
        Arc::clone(stats),
        Arc::clone(ledger),
        Arc::clone(unchecked),
        network_params,
        Arc::clone(channels),
        node_id,
        notify,
    )))))
}

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
