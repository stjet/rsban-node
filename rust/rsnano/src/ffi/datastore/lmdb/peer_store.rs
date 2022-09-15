use std::sync::Arc;

use crate::{
    datastore::{lmdb::LmdbPeerStore, PeerStore},
    Amount,
};

use super::{
    iterator::{to_lmdb_iterator_handle, LmdbIteratorHandle},
    lmdb_env::LmdbEnvHandle,
    TransactionHandle,
};

pub struct LmdbPeerStoreHandle(LmdbPeerStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbPeerStoreHandle {
    Box::into_raw(Box::new(LmdbPeerStoreHandle(LmdbPeerStore::new(
        Arc::clone(&*env_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_destroy(handle: *mut LmdbPeerStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_table_handle(handle: *mut LmdbPeerStoreHandle) -> u32 {
    (*handle).0.table_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_set_table_handle(
    handle: *mut LmdbPeerStoreHandle,
    table_handle: u32,
) {
    (*handle).0.table_handle = table_handle;
}
