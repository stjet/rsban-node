use std::{slice, sync::Arc};

use crate::{datastore::lmdb::LmdbBlockStore, BlockHash};

use super::{lmdb_env::LmdbEnvHandle, TransactionHandle};

pub struct LmdbBlockStoreHandle(LmdbBlockStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbBlockStoreHandle {
    Box::into_raw(Box::new(LmdbBlockStoreHandle(LmdbBlockStore::new(
        Arc::clone(&*env_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_destroy(handle: *mut LmdbBlockStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_blocks_handle(
    handle: *mut LmdbBlockStoreHandle,
) -> u32 {
    (*handle).0.blocks_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_set_blocks_handle(
    handle: *mut LmdbBlockStoreHandle,
    dbi: u32,
) {
    (*handle).0.blocks_handle = dbi;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_raw_put(
    handle: *mut LmdbBlockStoreHandle,
    txn: *mut TransactionHandle,
    data: *const u8,
    len: usize,
    hash: *const u8,
) {
    let txn = (*txn).as_write_txn();
    let data = slice::from_raw_parts(data, len);
    let hash = BlockHash::from_ptr(hash);
    (*handle).0.raw_put(txn, data, hash);
}
