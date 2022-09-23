use crate::{datastore::lmdb::LmdbWalletStore, ffi::copy_raw_key_bytes, RawKey};

pub struct LmdbWalletStoreHandle(LmdbWalletStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallet_store_create(fanout: usize) -> *mut LmdbWalletStoreHandle {
    Box::into_raw(Box::new(LmdbWalletStoreHandle(LmdbWalletStore::new(
        fanout,
    ))))
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

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallet_store_wallet_key_mem(
    handle: *mut LmdbWalletStoreHandle,
    key: *mut u8,
) {
    let k = (*handle).0.fans.lock().unwrap().wallet_key_mem.value();
    copy_raw_key_bytes(k, key);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallet_store_set_wallet_key_mem(
    handle: *mut LmdbWalletStoreHandle,
    key: *const u8,
) {
    (*handle)
        .0
        .fans
        .lock()
        .unwrap()
        .wallet_key_mem
        .value_set(RawKey::from_ptr(key));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallet_store_password(
    handle: *mut LmdbWalletStoreHandle,
    password: *mut u8,
) {
    let k = (*handle).0.fans.lock().unwrap().password.value();
    copy_raw_key_bytes(k, password);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallet_store_set_password(
    handle: *mut LmdbWalletStoreHandle,
    password: *const u8,
) {
    (*handle)
        .0
        .fans
        .lock()
        .unwrap()
        .password
        .value_set(RawKey::from_ptr(password));
}
