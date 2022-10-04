use std::ffi::{c_char, CStr};

use crate::{datastore::lmdb::LmdbWallets, ffi::U256ArrayDto};

use super::{
    iterator::{to_lmdb_iterator_handle, LmdbIteratorHandle},
    store::LmdbStoreHandle,
    TransactionHandle,
};

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
pub unsafe extern "C" fn rsn_lmdb_wallets_set_send_action_ids_handle(
    handle: *mut LmdbWalletsHandle,
    db_handle: u32,
) {
    (*handle).0.send_action_ids_handle = db_handle;
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
pub unsafe extern "C" fn rsn_lmdb_wallets_move_table(
    handle: *mut LmdbWalletsHandle,
    name: *const c_char,
    txn_source: &mut TransactionHandle,
    txn_destination: &mut TransactionHandle,
) {
    (*handle)
        .0
        .move_table(
            CStr::from_ptr(name).to_str().unwrap(),
            (*txn_source).as_txn(),
            (*txn_destination).as_txn(),
        )
        .unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_split_if_needed(
    handle: *mut LmdbWalletsHandle,
    txn_destination: &mut TransactionHandle,
    store: &mut LmdbStoreHandle,
) {
    (*handle)
        .0
        .split_if_needed((*txn_destination).as_txn(), &(*store))
        .unwrap();
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
