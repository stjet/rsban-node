use std::ops::Deref;

use crate::{datastore::lmdb::AccountStore, ffi::AccountInfoHandle, Account};

use super::TransactionHandle;

pub struct LmdbAccountStoreHandle(AccountStore);

#[no_mangle]
pub extern "C" fn rsn_lmdb_account_store_create() -> *mut LmdbAccountStoreHandle {
    Box::into_raw(Box::new(LmdbAccountStoreHandle(AccountStore::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_destroy(handle: *mut LmdbAccountStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_accounts_handle(
    handle: *mut LmdbAccountStoreHandle,
) -> u32 {
    (*handle).0.accounts_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_open_databases(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
    flags: u32,
) -> bool {
    (*handle).0.open_databases((*txn).as_txn(), flags).is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_put(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
    info: *const AccountInfoHandle,
) -> bool {
    let account = Account::from(account);
    let info = (*info).deref();
    (*handle)
        .0
        .put((*txn).as_write_tx(), &account, info)
        .is_ok()
}
