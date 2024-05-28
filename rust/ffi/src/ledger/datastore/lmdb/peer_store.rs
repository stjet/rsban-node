use super::TransactionHandle;
use rsnano_store_lmdb::LmdbPeerStore;
use std::sync::Arc;

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
