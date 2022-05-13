use crate::BlockProcessor;

pub struct BlockProcessorHandle(BlockProcessor);

#[no_mangle]
pub extern "C" fn rsn_block_processor_create() -> *mut BlockProcessorHandle {
    let processor = BlockProcessor::new();
    Box::into_raw(Box::new(BlockProcessorHandle(processor)))
}

#[no_mangle]
pub extern "C" fn rsn_state_block_signature_verification_destroy(
    handle: *mut BlockProcessorHandle,
) {
    drop(unsafe { Box::from_raw(handle) });
}