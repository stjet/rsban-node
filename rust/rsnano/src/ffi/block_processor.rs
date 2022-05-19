use std::{ffi::c_void, sync::Arc};

use crate::BlockProcessor;

use super::unchecked_info::UncheckedInfoHandle;

pub struct BlockProcessorHandle(Arc<BlockProcessor>);

#[no_mangle]
pub extern "C" fn rsn_block_processor_create(handle: *mut c_void) -> *mut BlockProcessorHandle {
    let processor = BlockProcessor::new(handle);
    Box::into_raw(Box::new(BlockProcessorHandle(Arc::new(processor))))
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_destroy(handle: *mut BlockProcessorHandle) {
    drop(unsafe { Box::from_raw(handle) });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_block_processor_add(f: BlockProcessorAddCallback) {
    BLOCKPROCESSOR_ADD_CALLBACK = Some(f);
}

type BlockProcessorAddCallback = unsafe extern "C" fn(*mut c_void, *mut UncheckedInfoHandle);
pub(crate) static mut BLOCKPROCESSOR_ADD_CALLBACK: Option<BlockProcessorAddCallback> = None;
