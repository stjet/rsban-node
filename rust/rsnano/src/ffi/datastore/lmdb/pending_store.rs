use std::{slice, sync::Arc};

use crate::{
    datastore::{lmdb::LmdbPendingStore, PendingStore},
    EndpointKey,
};

use super::{
    iterator::{to_lmdb_iterator_handle, LmdbIteratorHandle},
    lmdb_env::LmdbEnvHandle,
    TransactionHandle,
};

pub struct LmdbPendingStoreHandle(LmdbPendingStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_pending_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbPendingStoreHandle {
    Box::into_raw(Box::new(LmdbPendingStoreHandle(LmdbPendingStore::new(
        Arc::clone(&*env_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_pending_store_destroy(handle: *mut LmdbPendingStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_pending_store_table_handle(
    handle: *mut LmdbPendingStoreHandle,
) -> u32 {
    (*handle).0.table_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_pending_store_set_table_handle(
    handle: *mut LmdbPendingStoreHandle,
    table_handle: u32,
) {
    (*handle).0.table_handle = table_handle;
}
