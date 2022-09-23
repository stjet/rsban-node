use crate::datastore::lmdb::LmdbWalletStore;

pub struct LmdbWalletStoreHandle(LmdbWalletStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallet_store_create() -> *mut LmdbWalletStoreHandle {
    Box::into_raw(Box::new(LmdbWalletStoreHandle(LmdbWalletStore::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallet_store_destroy(handle: *mut LmdbWalletStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallet_store_db_handle(
    handle: *mut LmdbWalletStoreHandle,
) -> u32 {
    (*handle).0.db_handle()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallet_store_set_db_handle(
    handle: *mut LmdbWalletStoreHandle,
    dbi: u32,
) {
    (*handle).0.set_db_handle(dbi);
}
