use std::{ffi::c_void, sync::Arc};

use crate::{
    datastore::{lmdb::LmdbUncheckedStore, UncheckedStore},
    ffi::{copy_hash_bytes, VoidPointerCallback},
    BlockHash,
};

use super::{
    iterator::{
        to_lmdb_iterator_handle, ForEachParCallback, ForEachParWrapper, LmdbIteratorHandle,
    },
    lmdb_env::LmdbEnvHandle,
    TransactionHandle,
};

pub struct LmdbUncheckedStoreHandle(LmdbUncheckedStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbUncheckedStoreHandle {
    Box::into_raw(Box::new(LmdbUncheckedStoreHandle(LmdbUncheckedStore::new(
        Arc::clone(&*env_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_destroy(handle: *mut LmdbUncheckedStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_table_handle(
    handle: *mut LmdbUncheckedStoreHandle,
) -> u32 {
    (*handle).0.table_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_set_table_handle(
    handle: *mut LmdbUncheckedStoreHandle,
    table_handle: u32,
) {
    (*handle).0.table_handle = table_handle;
}
