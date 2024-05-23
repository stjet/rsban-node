use crate::block_processing::BlockProcessorHandle;
use rsnano_node::consensus::{ProcessLiveDispatcher, ProcessLiveDispatcherExt};
use std::sync::Arc;

pub struct ProcessLiveDispatcherHandle(pub Arc<ProcessLiveDispatcher>);

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
