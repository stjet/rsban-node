use crate::{
    block_processing::BlockProcessorHandle,
    ledger::datastore::LedgerHandle,
    messages::MessageHandle,
    transport::{ChannelHandle, TcpChannelsHandle},
    utils::ContainerInfoComponentHandle,
    NetworkConstantsDto, NodeConfigDto, StatHandle,
};
use rsnano_messages::Message;
use rsnano_node::{
    bootstrap::{BootstrapAscending, BootstrapAscendingExt},
    config::NodeConfig,
};
use std::{
    ffi::{c_char, CStr},
    sync::Arc,
};

pub struct BootstrapAscendingHandle(Arc<BootstrapAscending>);

#[no_mangle]
pub extern "C" fn rsn_bootstrap_ascending_create(
    block_processor: &BlockProcessorHandle,
    ledger: &LedgerHandle,
    stats: &StatHandle,
    channels: &TcpChannelsHandle,
    config: &NodeConfigDto,
    network_constants: &NetworkConstantsDto,
) -> *mut BootstrapAscendingHandle {
    let config: NodeConfig = config.try_into().unwrap();
    Box::into_raw(Box::new(BootstrapAscendingHandle(Arc::new(
        BootstrapAscending::new(
            Arc::clone(block_processor),
            Arc::clone(ledger),
            Arc::clone(stats),
            Arc::clone(channels),
            config,
            network_constants.try_into().unwrap(),
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_ascending_destroy(handle: *mut BootstrapAscendingHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_ascending_initialize(handle: &BootstrapAscendingHandle) {
    handle.0.initialize();
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_ascending_start(handle: &BootstrapAscendingHandle) {
    handle.0.start();
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_ascending_stop(handle: &BootstrapAscendingHandle) {
    handle.0.stop();
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_ascending_process(
    handle: &BootstrapAscendingHandle,
    message: &MessageHandle,
    channel: &ChannelHandle,
) {
    let Message::AscPullAck(payload) = &message.message else {
        panic!("not an asc pull ack message");
    };
    handle.0.process(payload, channel)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_ascending_collect_container_info(
    handle: &BootstrapAscendingHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = handle
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
