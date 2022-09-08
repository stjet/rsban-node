use std::{slice, sync::Arc};

use crate::{
    datastore::{
        lmdb::{LmdbBlockStore, MdbVal},
        BlockStore,
    },
    ffi::{copy_hash_bytes, BlockHandle},
    BlockHash,
};

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
    (*handle).0.raw_put(txn, data, &hash);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_block_raw_get(
    handle: *mut LmdbBlockStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
    value: *mut MdbVal,
) {
    let txn = (*txn).as_txn();
    let hash = BlockHash::from_ptr(hash);
    (*handle).0.block_raw_get(txn, &hash, &mut *value);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_block_store_put(
    handle: *mut LmdbBlockStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
    block: *mut BlockHandle,
) {
    (*handle).0.put(
        (*txn).as_write_txn(),
        &BlockHash::from_ptr(hash),
        (*block).block.read().unwrap().as_block(),
    );
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
        .successor((*txn).as_txn(), &BlockHash::from_ptr(hash));
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
