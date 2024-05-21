use super::priority_scheduler::ElectionSchedulerHandle;
use crate::{
    block_processing::BlockProcessorHandle, ledger::datastore::LedgerHandle,
    websocket::WebsocketListenerHandle,
};
use rsnano_node::consensus::{ProcessLiveDispatcher, ProcessLiveDispatcherExt};
use std::sync::Arc;

pub struct ProcessLiveDispatcherHandle(pub Arc<ProcessLiveDispatcher>);

#[no_mangle]
pub unsafe extern "C" fn rsn_process_live_dispatcher_create(
    ledger: &LedgerHandle,
    scheduler: &ElectionSchedulerHandle,
    websocket: *const WebsocketListenerHandle,
) -> *mut ProcessLiveDispatcherHandle {
    let websocket = if websocket.is_null() {
        None
    } else {
        Some(Arc::clone(&*websocket))
    };
    Box::into_raw(Box::new(ProcessLiveDispatcherHandle(Arc::new(
        ProcessLiveDispatcher::new(Arc::clone(ledger), Arc::clone(scheduler), websocket),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_process_live_dispatcher_destroy(
    handle: *mut ProcessLiveDispatcherHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_process_live_dispatcher_connect(
    handle: &ProcessLiveDispatcherHandle,
    block_processor: &BlockProcessorHandle,
) {
    handle.0.connect(block_processor);
}
