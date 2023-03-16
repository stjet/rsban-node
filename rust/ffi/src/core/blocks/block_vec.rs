use std::sync::{Arc, RwLock};

use rsnano_core::BlockEnum;

use super::BlockHandle;

pub struct BlockVecHandle(pub Vec<Arc<RwLock<BlockEnum>>>);

#[no_mangle]
pub extern "C" fn rsn_block_vec_create() -> *mut BlockVecHandle {
    Box::into_raw(Box::new(BlockVecHandle(Vec::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_vec_destroy(handle: *mut BlockVecHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_vec_erase_last(handle: *mut BlockVecHandle, count: usize) {
    (*handle).0.truncate((*handle).0.len() - count);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_vec_push_back(
    handle: *mut BlockVecHandle,
    block: *const BlockHandle,
) {
    (*handle).0.push((*block).block.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_vec_size(handle: *mut BlockVecHandle) -> usize {
    (*handle).0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_vec_clear(handle: *mut BlockVecHandle) {
    (*handle).0.clear();
}
