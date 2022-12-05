use rsnano_node::block_processing::{BlockProcessor, BLOCKPROCESSOR_ADD_CALLBACK};
use std::{ffi::c_void, ops::Deref, sync::Arc};

use crate::core::UncheckedInfoHandle;

pub struct BlockProcessorHandle(Arc<BlockProcessor>);

impl Deref for BlockProcessorHandle {
    type Target = Arc<BlockProcessor>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_create(handle: *mut c_void) -> *mut BlockProcessorHandle {
    let processor = BlockProcessor::new(handle);
    Box::into_raw(Box::new(BlockProcessorHandle(Arc::new(processor))))
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_destroy(handle: *mut BlockProcessorHandle) {
    drop(unsafe { Box::from_raw(handle) });
}

pub type BlockProcessorAddCallback = unsafe extern "C" fn(*mut c_void, *mut UncheckedInfoHandle);
static mut ADD_CALLBACK: Option<BlockProcessorAddCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_block_processor_add(f: BlockProcessorAddCallback) {
    ADD_CALLBACK = Some(f);
    BLOCKPROCESSOR_ADD_CALLBACK = Some(|handle, info| {
        ADD_CALLBACK.expect("ADD_CALLBACK missing")(
            handle,
            Box::into_raw(Box::new(UncheckedInfoHandle::new(info.clone()))),
        )
    });
}
