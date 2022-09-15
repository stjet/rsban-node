use std::sync::Arc;

use crate::datastore::lmdb::LmdbOnlineWeightStore;

use super::lmdb_env::LmdbEnvHandle;

pub struct LmdbOnlineWeightStoreHandle(LmdbOnlineWeightStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbOnlineWeightStoreHandle {
    Box::into_raw(Box::new(LmdbOnlineWeightStoreHandle(
        LmdbOnlineWeightStore::new(Arc::clone(&*env_handle)),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_destroy(
    handle: *mut LmdbOnlineWeightStoreHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_table_handle(
    handle: *mut LmdbOnlineWeightStoreHandle,
) -> u32 {
    (*handle).0.table_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_online_weight_store_set_table_handle(
    handle: *mut LmdbOnlineWeightStoreHandle,
    table_handle: u32,
) {
    (*handle).0.table_handle = table_handle;
}
