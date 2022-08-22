use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use crate::{
    datastore::{lmdb::LmdbAccountStore, AccountStore},
    ffi::AccountInfoHandle,
    Account,
};

use super::{iterator::LmdbIteratorHandle, lmdb_env::LmdbEnvHandle, TransactionHandle};

pub struct LmdbAccountStoreHandle(LmdbAccountStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbAccountStoreHandle {
    Box::into_raw(Box::new(LmdbAccountStoreHandle(LmdbAccountStore::new(
        Arc::clone(&*env_handle),
    ))))
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
) {
    let account = Account::from_ptr(account);
    let info = (*info).deref();
    (*handle).0.put((*txn).as_write_txn(), &account, info);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_get(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
    info: *mut AccountInfoHandle,
) -> bool {
    let account = Account::from_ptr(account);
    let info = (*info).deref_mut();
    match (*handle).0.get((*txn).as_txn(), &account) {
        Some(i) => {
            *info = i;
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_del(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
) {
    let account = Account::from_ptr(account);
    (*handle).0.del((*txn).as_write_txn(), &account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_begin_account(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
) -> *mut LmdbIteratorHandle {
    let account = Account::from_ptr(account);
    let mut iterator = (*handle).0.begin_account((*txn).as_txn(), &account);
    LmdbIteratorHandle::new(iterator.take_lmdb_raw_iterator())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_begin(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
) -> *mut LmdbIteratorHandle {
    let mut iterator = (*handle).0.begin((*txn).as_txn());
    LmdbIteratorHandle::new(iterator.take_lmdb_raw_iterator())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_rbegin(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
) -> *mut LmdbIteratorHandle {
    let mut iterator = (*handle).0.rbegin((*txn).as_txn());
    LmdbIteratorHandle::new(iterator.take_lmdb_raw_iterator())
}
