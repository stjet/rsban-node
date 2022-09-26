use std::{
    ffi::{c_char, CStr},
    path::PathBuf,
    str::FromStr,
};

use crate::{
    datastore::lmdb::{LmdbWalletStore, WalletValue},
    ffi::copy_raw_key_bytes,
    Account, RawKey,
};

use super::TransactionHandle;

pub struct LmdbWalletStoreHandle(LmdbWalletStore);

#[repr(C)]
pub struct WalletValueDto {
    pub key: [u8; 32],
    pub work: u64,
}

impl From<WalletValue> for WalletValueDto {
    fn from(value: WalletValue) -> Self {
        WalletValueDto {
            key: *value.key.as_bytes(),
            work: value.work,
        }
    }
}

impl From<&WalletValueDto> for WalletValue {
    fn from(dto: &WalletValueDto) -> Self {
        WalletValue::new(RawKey::from_bytes(dto.key), dto.work)
    }
}

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
pub unsafe extern "C" fn rsn_lmdb_wallet_store_initialize(
    handle: *mut LmdbWalletStoreHandle,
    txn: *mut TransactionHandle,
    path: *const c_char,
) -> bool {
    let p = PathBuf::from_str(CStr::from_ptr(path).to_str().unwrap()).unwrap();
    (*handle).0.initialize((*txn).as_txn(), &p).is_ok()
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

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallet_store_entry_put_raw(
    handle: *mut LmdbWalletStoreHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
    entry: *const WalletValueDto,
) {
    (*handle).0.entry_put_raw(
        (*txn).as_txn(),
        &Account::from_ptr(account),
        &WalletValue::from(&*entry),
    )
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallet_store_entry_get_raw(
    handle: *mut LmdbWalletStoreHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
    result: *mut WalletValueDto,
) {
    let entry = (*handle)
        .0
        .entry_get_raw((*txn).as_txn(), &Account::from_ptr(account));
    *result = entry.into()
}
