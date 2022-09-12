use std::sync::Arc;

use crate::{
    datastore::{lmdb::LmdbFinalVoteStore, FinalVoteStore},
    BlockHash, QualifiedRoot,
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
