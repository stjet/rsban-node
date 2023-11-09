use super::TransactionHandle;
use crate::{copy_hash_bytes, core::BlockHandle};
use rsnano_core::BlockHash;
use rsnano_store_lmdb::LmdbBlockStore;
use std::{ptr, slice, sync::Arc};

pub struct LmdbBlockStoreHandle(Arc<LmdbBlockStore>);

impl LmdbBlockStoreHandle {
    pub fn new(store: Arc<LmdbBlockStore>) -> *mut Self {
        Box::into_raw(Box::new(LmdbBlockStoreHandle(store)))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_destroy(handle: *mut LmdbBlockStoreHandle) {
    drop(Box::from_raw(handle))
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
    (*handle).0.raw_put(txn, data, &hash);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_put(
    handle: *mut LmdbBlockStoreHandle,
    txn: *mut TransactionHandle,
    _hash: *const u8,
    block: &BlockHandle,
) {
    (*handle).0.put((*txn).as_write_txn(), &block);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_exists(
    handle: *mut LmdbBlockStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
) -> bool {
    (*handle)
        .0
        .exists((*txn).as_txn(), &BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_successor(
    handle: *mut LmdbBlockStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
    result: *mut u8,
) {
    let successor = (*handle)
        .0
        .successor((*txn).as_txn(), &BlockHash::from_ptr(hash))
        .unwrap_or_default();
    copy_hash_bytes(successor, result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_successor_clear(
    handle: *mut LmdbBlockStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
) {
    (*handle)
        .0
        .successor_clear((*txn).as_write_txn(), &BlockHash::from_ptr(hash));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_get(
    handle: *mut LmdbBlockStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
) -> *mut BlockHandle {
    match (*handle).0.get((*txn).as_txn(), &BlockHash::from_ptr(hash)) {
        Some(block) => Box::into_raw(Box::new(BlockHandle(Arc::new(block)))),
        None => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_del(
    handle: *mut LmdbBlockStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
) {
    (*handle)
        .0
        .del((*txn).as_write_txn(), &BlockHash::from_ptr(hash));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_count(
    handle: *mut LmdbBlockStoreHandle,
    txn: *mut TransactionHandle,
) -> u64 {
    (*handle).0.count((*txn).as_txn())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_random(
    handle: *mut LmdbBlockStoreHandle,
    txn: *mut TransactionHandle,
) -> *mut BlockHandle {
    match (*handle).0.random((*txn).as_txn()) {
        Some(block) => Box::into_raw(Box::new(BlockHandle(Arc::new(block)))),
        None => ptr::null_mut(),
    }
}
