use std::sync::Arc;

use crate::{
    datastore::{lmdb::LmdbFrontierStore, FrontierStore},
    ffi::copy_account_bytes,
    Account, BlockHash,
};

use super::{lmdb_env::LmdbEnvHandle, TransactionHandle};

pub struct LmdbFrontierStoreHandle(LmdbFrontierStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbFrontierStoreHandle {
    Box::into_raw(Box::new(LmdbFrontierStoreHandle(LmdbFrontierStore::new(
        Arc::clone(&*env_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_destroy(handle: *mut LmdbFrontierStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_table_handle(
    handle: *mut LmdbFrontierStoreHandle,
) -> u32 {
    (*handle).0.table_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_set_table_handle(
    handle: *mut LmdbFrontierStoreHandle,
    table_handle: u32,
) {
    (*handle).0.table_handle = table_handle;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_put(
    handle: *mut LmdbFrontierStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
    account: *const u8,
) {
    (*handle).0.put(
        (*txn).as_write_txn(),
        &BlockHash::from_ptr(hash),
        &Account::from_ptr(account),
    );
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_get(
    handle: *mut LmdbFrontierStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
    account: *mut u8,
) {
    let result = (*handle).0.get((*txn).as_txn(), &BlockHash::from_ptr(hash));
    copy_account_bytes(result, account);
}
