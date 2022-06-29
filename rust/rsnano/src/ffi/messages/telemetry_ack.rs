use std::{
    ffi::c_void,
    time::{Duration, SystemTime},
};

use crate::{
    ffi::{copy_account_bytes, copy_hash_bytes, copy_signature_bytes, FfiStream},
    messages::TelemetryData,
    Account, BlockHash, Signature,
};

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

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_signature(
    handle: *mut TelemetryDataHandle,
    signature: *mut u8,
) {
    copy_signature_bytes(&(*handle).0.signature, signature);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_signature(
    handle: *mut TelemetryDataHandle,
    signature: *const u8,
) {
    (*handle).0.signature = Signature::from(signature);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_node_id(
    handle: *mut TelemetryDataHandle,
    node_id: *mut u8,
) {
    copy_account_bytes((*handle).0.node_id, node_id);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_node_id(
    handle: *mut TelemetryDataHandle,
    node_id: *const u8,
) {
    (*handle).0.node_id = Account::from(node_id);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_block_count(
    handle: *mut TelemetryDataHandle,
) -> u64 {
    (*handle).0.block_count
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_block_count(
    handle: *mut TelemetryDataHandle,
    count: u64,
) {
    (*handle).0.block_count = count;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_cemented_count(
    handle: *mut TelemetryDataHandle,
) -> u64 {
    (*handle).0.cemented_count
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_cemented_count(
    handle: *mut TelemetryDataHandle,
    count: u64,
) {
    (*handle).0.cemented_count = count;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_unchecked_count(
    handle: *mut TelemetryDataHandle,
) -> u64 {
    (*handle).0.unchecked_count
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_unchecked_count(
    handle: *mut TelemetryDataHandle,
    count: u64,
) {
    (*handle).0.unchecked_count = count;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_account_count(
    handle: *mut TelemetryDataHandle,
) -> u64 {
    (*handle).0.account_count
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_account_count(
    handle: *mut TelemetryDataHandle,
    count: u64,
) {
    (*handle).0.account_count = count;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_bandwidth_cap(
    handle: *mut TelemetryDataHandle,
) -> u64 {
    (*handle).0.bandwidth_cap
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_bandwidth_cap(
    handle: *mut TelemetryDataHandle,
    cap: u64,
) {
    (*handle).0.bandwidth_cap = cap;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_uptime(handle: *mut TelemetryDataHandle) -> u64 {
    (*handle).0.uptime
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_uptime(
    handle: *mut TelemetryDataHandle,
    uptime: u64,
) {
    (*handle).0.uptime = uptime;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_peer_count(
    handle: *mut TelemetryDataHandle,
) -> u32 {
    (*handle).0.peer_count
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_peer_count(
    handle: *mut TelemetryDataHandle,
    count: u32,
) {
    (*handle).0.peer_count = count;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_protocol_version(
    handle: *mut TelemetryDataHandle,
) -> u8 {
    (*handle).0.protocol_version
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_protocol_version(
    handle: *mut TelemetryDataHandle,
    version: u8,
) {
    (*handle).0.protocol_version = version;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_genesis_block(
    handle: *mut TelemetryDataHandle,
    block: *mut u8,
) {
    copy_hash_bytes((*handle).0.genesis_block, block);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_genesis_block(
    handle: *mut TelemetryDataHandle,
    block: *const u8,
) {
    (*handle).0.genesis_block = BlockHash::from(block);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_major_version(
    handle: *mut TelemetryDataHandle,
) -> u8 {
    (*handle).0.major_version
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_major_version(
    handle: *mut TelemetryDataHandle,
    version: u8,
) {
    (*handle).0.major_version = version;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_minor_version(
    handle: *mut TelemetryDataHandle,
) -> u8 {
    (*handle).0.minor_version
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_minor_version(
    handle: *mut TelemetryDataHandle,
    version: u8,
) {
    (*handle).0.minor_version = version;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_patch_version(
    handle: *mut TelemetryDataHandle,
) -> u8 {
    (*handle).0.patch_version
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_patch_version(
    handle: *mut TelemetryDataHandle,
    version: u8,
) {
    (*handle).0.patch_version = version;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_pre_release_version(
    handle: *mut TelemetryDataHandle,
) -> u8 {
    (*handle).0.pre_release_version
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_pre_release_version(
    handle: *mut TelemetryDataHandle,
    version: u8,
) {
    (*handle).0.pre_release_version = version;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_maker(handle: *mut TelemetryDataHandle) -> u8 {
    (*handle).0.maker
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_maker(handle: *mut TelemetryDataHandle, maker: u8) {
    (*handle).0.maker = maker;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_timestamp_ms(
    handle: *mut TelemetryDataHandle,
) -> u64 {
    (*handle)
        .0
        .timestamp
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_timestamp(
    handle: *mut TelemetryDataHandle,
    timestamp_ms: u64,
) {
    (*handle).0.timestamp = SystemTime::UNIX_EPOCH + Duration::from_millis(timestamp_ms);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_active_difficulty(
    handle: *mut TelemetryDataHandle,
) -> u64 {
    (*handle).0.active_difficulty
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_active_difficulty(
    handle: *mut TelemetryDataHandle,
    difficulty: u64,
) {
    (*handle).0.active_difficulty = difficulty;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_unknown_data_len(
    handle: *mut TelemetryDataHandle,
) -> usize {
    (*handle).0.unknown_data.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_get_unknown_data(
    handle: *mut TelemetryDataHandle,
    data: *mut u8,
) {
    let source = &(*handle).0.unknown_data;
    let target = std::slice::from_raw_parts_mut(data, source.len());
    target.copy_from_slice(source)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_set_unknown_data(
    handle: *mut TelemetryDataHandle,
    data: *const u8,
    len: usize,
) {
    (*handle).0.unknown_data = std::slice::from_raw_parts(data, len)
        .iter()
        .cloned()
        .collect()
}

#[no_mangle]
pub extern "C" fn rsn_telemetry_data_size() -> usize {
    TelemetryData::serialized_size_without_unknown_data()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_telemetry_data_deserialize(
    handle: *mut TelemetryDataHandle,
    stream: *mut c_void,
    payload_len: u16,
) -> bool {
    let mut stream = FfiStream::new(stream);
    (*handle).0.deserialize(&mut stream, payload_len).is_ok()
}
