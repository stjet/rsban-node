use super::BlockProcessorHandle;
use crate::{
    cementation::ConfirmingSetHandle, ledger::datastore::LedgerHandle,
    representatives::RepresentativeRegisterHandle, transport::TcpChannelsHandle,
    utils::ContainerInfoComponentHandle, StatHandle,
};
use rsnano_node::block_processing::{LocalBlockBroadcaster, LocalBlockBroadcasterExt};
use std::{
    ffi::{c_char, CStr},
    sync::Arc,
};

pub struct LocalBlockBroadcasterHandle(Arc<LocalBlockBroadcaster>);

#[no_mangle]
pub extern "C" fn rsn_local_block_broadcaster_create(
    block_processor: &BlockProcessorHandle,
    stats: &StatHandle,
    channels: &TcpChannelsHandle,
    representatives: &RepresentativeRegisterHandle,
    ledger: &LedgerHandle,
    confirming_set: &ConfirmingSetHandle,
    enabled: bool,
) -> *mut LocalBlockBroadcasterHandle {
    Box::into_raw(Box::new(LocalBlockBroadcasterHandle(Arc::new(
        LocalBlockBroadcaster::new(
            Arc::clone(&block_processor),
            Arc::clone(&stats),
            Arc::clone(&channels),
            Arc::clone(&representatives),
            Arc::clone(&ledger),
            Arc::clone(&confirming_set),
            enabled,
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_block_broadcaster_destroy(
    handle: *mut LocalBlockBroadcasterHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_block_broadcaster_initialize(
    handle: &LocalBlockBroadcasterHandle,
) {
    handle.0.initialize();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_block_broadcaster_start(handle: &LocalBlockBroadcasterHandle) {
    handle.0.start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_block_broadcaster_stop(handle: &LocalBlockBroadcasterHandle) {
    handle.0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_local_block_broadcaster_collect_container_info(
    handle: &LocalBlockBroadcasterHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = handle
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
