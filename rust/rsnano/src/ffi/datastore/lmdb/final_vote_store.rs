use std::{ffi::c_void, sync::Arc};

use crate::{
    datastore::{lmdb::LmdbFinalVoteStore, FinalVoteStore},
    BlockHash, QualifiedRoot, Root,
};

use super::{
    iterator::{to_lmdb_iterator_handle, LmdbIteratorHandle},
    lmdb_env::LmdbEnvHandle,
    TransactionHandle,
};

pub struct LmdbFinalVoteStoreHandle(LmdbFinalVoteStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_final_vote_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbFinalVoteStoreHandle {
    Box::into_raw(Box::new(LmdbFinalVoteStoreHandle(LmdbFinalVoteStore::new(
        Arc::clone(&*env_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_final_vote_store_destroy(handle: *mut LmdbFinalVoteStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_final_vote_store_table_handle(
    handle: *mut LmdbFinalVoteStoreHandle,
) -> u32 {
    (*handle).0.table_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_final_vote_store_set_table_handle(
    handle: *mut LmdbFinalVoteStoreHandle,
    table_handle: u32,
) {
    (*handle).0.table_handle = table_handle;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_final_vote_store_put(
    handle: *mut LmdbFinalVoteStoreHandle,
    txn: *mut TransactionHandle,
    root: *const u8,
    hash: *const u8,
) -> bool {
    (*handle).0.put(
        (*txn).as_write_txn(),
        &QualifiedRoot::from_ptr(root),
        &BlockHash::from_ptr(hash),
    )
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_final_vote_store_begin(
    handle: *mut LmdbFinalVoteStoreHandle,
    txn: *mut TransactionHandle,
) -> *mut LmdbIteratorHandle {
    let mut iterator = (*handle).0.begin((*txn).as_txn());
    to_lmdb_iterator_handle(iterator.as_mut())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_final_vote_store_begin_at_root(
    handle: *mut LmdbFinalVoteStoreHandle,
    txn: *mut TransactionHandle,
    root: *const u8,
) -> *mut LmdbIteratorHandle {
    let root = QualifiedRoot::from_ptr(root);
    let mut iterator = (*handle).0.begin_at_root((*txn).as_txn(), &root);
    to_lmdb_iterator_handle(iterator.as_mut())
}

#[repr(C)]
pub struct BlockHashArrayDto {
    pub data: *const u8,
    pub count: usize,
    pub raw_data: *mut c_void,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_final_vote_store_get(
    handle: *mut LmdbFinalVoteStoreHandle,
    txn: *mut TransactionHandle,
    root: *const u8,
    result: *mut BlockHashArrayDto,
) {
    let hashes = (*handle).0.get((*txn).as_txn(), Root::from_ptr(root));
    let mut bytes = Box::new(Vec::with_capacity(hashes.len() * 32));
    for h in &hashes {
        for &b in h.as_bytes() {
            bytes.push(b);
        }
    }
    (*result).count = bytes.len();
    (*result).data = bytes.as_ptr();
    (*result).raw_data = Box::into_raw(bytes) as *mut c_void;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_hash_array_destroy(data: *mut BlockHashArrayDto) {
    let v = (*data).raw_data as *mut Vec<u8>;
    drop(Box::from_raw(v))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_final_vote_store_del(
    handle: *mut LmdbFinalVoteStoreHandle,
    txn: *mut TransactionHandle,
    root: *const u8,
) {
    (*handle).0.del((*txn).as_write_txn(), Root::from_ptr(root));
}
