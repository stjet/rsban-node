use crate::datastore::lmdb::LmdbWallets;

use super::TransactionHandle;

pub struct LmdbWalletsHandle(LmdbWallets);

#[no_mangle]
pub extern "C" fn rsn_lmdb_wallets_create() -> *mut LmdbWalletsHandle {
    Box::into_raw(Box::new(LmdbWalletsHandle(LmdbWallets::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_destroy(handle: *mut LmdbWalletsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_db_handle(handle: *mut LmdbWalletsHandle) -> u32 {
    (*handle).0.handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_set_db_handle(
    handle: *mut LmdbWalletsHandle,
    db_handle: u32,
) {
    (*handle).0.handle = db_handle;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_init(
    handle: *mut LmdbWalletsHandle,
    txn: &mut TransactionHandle,
) -> bool {
    (*handle).0.initialize((*txn).as_txn()).is_ok()
}
