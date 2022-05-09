use super::BlockHandle;
use crate::BlockUniquer;
use std::sync::Arc;

pub struct BlockUniquerHandle {
    uniquer: BlockUniquer,
}

#[no_mangle]
pub extern "C" fn rsn_block_uniquer_create() -> *mut BlockUniquerHandle {
    Box::into_raw(Box::new(BlockUniquerHandle {
        uniquer: BlockUniquer::new(),
    }))
}

#[no_mangle]
pub extern "C" fn rsn_block_uniquer_destroy(handle: *mut BlockUniquerHandle) {
    let uniquer = unsafe { Box::from_raw(handle) };
    drop(uniquer);
}

#[no_mangle]
pub extern "C" fn rsn_block_uniquer_size(handle: *const BlockUniquerHandle) -> usize {
    unsafe { &*handle }.uniquer.size()
}

#[no_mangle]
pub extern "C" fn rsn_block_uniquer_unique(
    handle: *mut BlockUniquerHandle,
    block: *mut BlockHandle,
) -> *mut BlockHandle {
    let original = &unsafe { &*block }.block;
    let uniqued = unsafe { &*handle }.uniquer.unique(original);
    if Arc::ptr_eq(&uniqued, original) {
        block
    } else {
        Box::into_raw(Box::new(BlockHandle { block: uniqued }))
    }
}
