use crate::core::BlockHandle;
use rsnano_node::block_processing::{
    BlockProcessor, BLOCKPROCESSOR_ADD_CALLBACK, BLOCKPROCESSOR_HALF_FULL_CALLBACK,
};
use std::{
    ffi::c_void,
    ops::Deref,
    sync::{Arc, MutexGuard},
};

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

pub struct BlockProcessorLockHandle(Option<MutexGuard<'static, ()>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_lock(
    handle: *mut BlockProcessorHandle,
) -> *mut BlockProcessorLockHandle {
    let guard = (*handle).mutex.lock().unwrap();
    Box::into_raw(Box::new(BlockProcessorLockHandle(Some(guard))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_lock_destroy(handle: *mut BlockProcessorLockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_lock_lock(
    handle: *mut BlockProcessorLockHandle,
    processor: *mut BlockProcessorHandle,
) {
    (*handle).0 = Some((*processor).0.mutex.lock().unwrap());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_lock_unlock(handle: *mut BlockProcessorLockHandle) {
    (*handle).0 = None;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_notify_all(handle: *mut BlockProcessorHandle) {
    (*handle).0.condition.notify_all();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_notify_one(handle: *mut BlockProcessorHandle) {
    (*handle).0.condition.notify_one();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_wait(
    handle: *mut BlockProcessorHandle,
    lock_handle: *mut BlockProcessorLockHandle,
) {
    let guard = (*lock_handle).0.take().unwrap();
    let guard = (*handle).0.condition.wait(guard).unwrap();
    (*lock_handle).0 = Some(guard);
}

pub type BlockProcessorAddCallback = unsafe extern "C" fn(*mut c_void, *mut BlockHandle);
pub type BlockProcessorHalfFullCallback = unsafe extern "C" fn(*mut c_void) -> bool;
static mut ADD_CALLBACK: Option<BlockProcessorAddCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_block_processor_add(f: BlockProcessorAddCallback) {
    ADD_CALLBACK = Some(f);
    BLOCKPROCESSOR_ADD_CALLBACK = Some(|handle, block| {
        ADD_CALLBACK.expect("ADD_CALLBACK missing")(
            handle,
            Box::into_raw(Box::new(BlockHandle::new(block))),
        )
    });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_block_processor_half_full(f: BlockProcessorHalfFullCallback) {
    BLOCKPROCESSOR_HALF_FULL_CALLBACK = Some(f);
}
