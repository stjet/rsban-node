use crate::{datastore::lmdb::LmdbWallets, ffi::U256ArrayDto};

use super::{store::LmdbStoreHandle, TransactionHandle};

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
pub unsafe extern "C" fn rsn_lmdb_wallets_send_action_ids_handle(
    handle: *mut LmdbWalletsHandle,
) -> u32 {
    (*handle).0.send_action_ids_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_init(
    handle: *mut LmdbWalletsHandle,
    txn: *mut TransactionHandle,
    store: *mut LmdbStoreHandle,
) -> bool {
    (*handle).0.initialize((*txn).as_txn(), &*store).is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_get_wallet_ids(
    handle: *mut LmdbWalletsHandle,
    txn: *mut TransactionHandle,
    result: *mut U256ArrayDto,
) {
    let wallet_ids = (*handle).0.get_wallet_ids((*txn).as_txn());
    let data = Box::new(wallet_ids.iter().map(|i| *i.as_bytes()).collect());
    (*result).initialize(data)
}
