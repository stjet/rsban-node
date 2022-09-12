use std::sync::Arc;

use crate::datastore::lmdb::LmdbFinalVoteStore;

use super::lmdb_env::LmdbEnvHandle;

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
