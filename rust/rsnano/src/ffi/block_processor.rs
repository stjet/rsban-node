use std::sync::Arc;

use crate::BlockProcessor;

pub struct BlockProcessorHandle(Arc<BlockProcessor>);

#[no_mangle]
pub extern "C" fn rsn_block_processor_create() -> *mut BlockProcessorHandle {
    let processor = BlockProcessor::new();
    Box::into_raw(Box::new(BlockProcessorHandle(Arc::new(processor))))
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_destroy(handle: *mut BlockProcessorHandle) {
    drop(unsafe { Box::from_raw(handle) });
}
