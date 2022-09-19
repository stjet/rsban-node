use std::sync::Arc;

use crate::datastore::{lmdb::LmdbVersionStore, VersionStore};

use super::{lmdb_env::LmdbEnvHandle, TransactionHandle};

pub struct LmdbVersionStoreHandle(LmdbVersionStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_version_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbVersionStoreHandle {
    Box::into_raw(Box::new(LmdbVersionStoreHandle(LmdbVersionStore::new(
        Arc::clone(&*env_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_version_store_destroy(handle: *mut LmdbVersionStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_version_store_table_handle(
    handle: *mut LmdbVersionStoreHandle,
) -> u32 {
    (*handle).0.table_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_version_store_set_table_handle(
    handle: *mut LmdbVersionStoreHandle,
    table_handle: u32,
) {
    (*handle).0.table_handle = table_handle;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_version_store_put(
    handle: *mut LmdbVersionStoreHandle,
    txn: *mut TransactionHandle,
    version: i32,
) {
    (*handle).0.put((*txn).as_write_txn(), version);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_version_store_get(
    handle: *mut LmdbVersionStoreHandle,
    txn: *mut TransactionHandle,
) -> i32 {
    (*handle).0.get((*txn).as_txn())
}
