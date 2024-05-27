use super::TransactionHandle;
use rsnano_store_lmdb::LmdbPeerStore;
use std::{net::SocketAddrV6, slice, sync::Arc};

pub struct LmdbPeerStoreHandle(Arc<LmdbPeerStore>);

impl LmdbPeerStoreHandle {
    pub fn new(store: Arc<LmdbPeerStore>) -> *mut Self {
        Box::into_raw(Box::new(LmdbPeerStoreHandle(store)))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_destroy(handle: *mut LmdbPeerStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_put(
    handle: &mut LmdbPeerStoreHandle,
    txn: &mut TransactionHandle,
    address: *const u8,
    port: u16,
) {
    let endpoint = to_socket_addr_v6(address, port);
    handle.0.put(txn.as_write_txn(), &endpoint);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_exists(
    handle: &mut LmdbPeerStoreHandle,
    txn: &mut TransactionHandle,
    address: *const u8,
    port: u16,
) -> bool {
    let endpoint = to_socket_addr_v6(address, port);
    handle.0.exists(txn.as_txn(), &endpoint)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_count(
    handle: &mut LmdbPeerStoreHandle,
    txn: &mut TransactionHandle,
) -> usize {
    handle.0.count(txn.as_txn()) as usize
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_peer_store_clear(
    handle: &mut LmdbPeerStoreHandle,
    txn: &mut TransactionHandle,
) {
    handle.0.clear(txn.as_write_txn())
}

unsafe fn to_socket_addr_v6(address: *const u8, port: u16) -> SocketAddrV6 {
    let ip_bytes: [u8; 16] = slice::from_raw_parts(address, 16).try_into().unwrap();
    SocketAddrV6::new(ip_bytes.into(), port, 0, 0)
}
