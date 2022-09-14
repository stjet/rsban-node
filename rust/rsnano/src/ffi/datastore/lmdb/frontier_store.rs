use std::sync::Arc;

use crate::datastore::{lmdb::LmdbFrontierStore, FrontierStore};

use super::lmdb_env::LmdbEnvHandle;

pub struct LmdbFrontierStoreHandle(LmdbFrontierStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbFrontierStoreHandle {
    Box::into_raw(Box::new(LmdbFrontierStoreHandle(LmdbFrontierStore::new(
        Arc::clone(&*env_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_destroy(handle: *mut LmdbFrontierStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_table_handle(
    handle: *mut LmdbFrontierStoreHandle,
) -> u32 {
    (*handle).0.table_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_set_table_handle(
    handle: *mut LmdbFrontierStoreHandle,
    table_handle: u32,
) {
    (*handle).0.table_handle = table_handle;
}
