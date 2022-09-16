use std::{slice, sync::Arc};

use crate::{
    datastore::{lmdb::LmdbPeerStore, PeerStore},
    EndpointKey,
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

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_put(
    handle: *mut LmdbPeerStoreHandle,
    txn: *mut TransactionHandle,
    address: *const u8,
    port: u16,
) {
    let endpoint = to_endpoint_key(address, port);
    (*handle).0.put((*txn).as_write_txn(), &endpoint);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_del(
    handle: *mut LmdbPeerStoreHandle,
    txn: *mut TransactionHandle,
    address: *const u8,
    port: u16,
) {
    let endpoint = to_endpoint_key(address, port);
    (*handle).0.del((*txn).as_write_txn(), &endpoint);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_exists(
    handle: *mut LmdbPeerStoreHandle,
    txn: *mut TransactionHandle,
    address: *const u8,
    port: u16,
) -> bool {
    let endpoint = to_endpoint_key(address, port);
    (*handle).0.exists((*txn).as_txn(), &endpoint)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_count(
    handle: *mut LmdbPeerStoreHandle,
    txn: *mut TransactionHandle,
) -> usize {
    (*handle).0.count((*txn).as_txn())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_clear(
    handle: *mut LmdbPeerStoreHandle,
    txn: *mut TransactionHandle,
) {
    (*handle).0.clear((*txn).as_write_txn())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_begin(
    handle: *mut LmdbPeerStoreHandle,
    txn: *mut TransactionHandle,
) -> *mut LmdbIteratorHandle {
    let mut iterator = (*handle).0.begin((*txn).as_txn());
    to_lmdb_iterator_handle(iterator.as_mut())
}

unsafe fn to_endpoint_key(address: *const u8, port: u16) -> EndpointKey {
    EndpointKey::new(slice::from_raw_parts(address, 16).try_into().unwrap(), port)
}
