use std::ffi::c_void;

use rsnano_core::BlockHash;

pub type NotifyBlockAlreadyCementedCallback = unsafe extern "C" fn(*mut c_void, *const u8);

pub type AwaitingProcessingSizeCallback = unsafe extern "C" fn(*mut c_void) -> u64;

//
// BlockHashVec
//

pub struct BlockHashVecHandle(pub Vec<BlockHash>);

#[no_mangle]
pub extern "C" fn rsn_block_hash_vec_create() -> *mut BlockHashVecHandle {
    Box::into_raw(Box::new(BlockHashVecHandle(Vec::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_hash_vec_clone(
    handle: *const BlockHashVecHandle,
) -> *mut BlockHashVecHandle {
    Box::into_raw(Box::new(BlockHashVecHandle((*handle).0.clone())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_hash_vec_destroy(handle: *mut BlockHashVecHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_hash_vec_size(handle: *mut BlockHashVecHandle) -> usize {
    (*handle).0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_hash_vec_push(handle: *mut BlockHashVecHandle, hash: *const u8) {
    (*handle).0.push(BlockHash::from_ptr(hash));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_hash_vec_clear(handle: *mut BlockHashVecHandle) {
    (*handle).0.clear();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_hash_vec_assign_range(
    destination: *mut BlockHashVecHandle,
    source: *const BlockHashVecHandle,
    start: usize,
    end: usize,
) {
    (*destination).0.clear();
    (*destination).0.extend_from_slice(&(*source).0[start..end]);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_hash_vec_truncate(
    handle: *mut BlockHashVecHandle,
    new_size: usize,
) {
    (*handle).0.truncate(new_size);
}
