use super::{lmdb_env::LmdbEnvHandle, wallet::WalletHandle, TransactionHandle};
use crate::{utils::ContextWrapper, U256ArrayDto, VoidPointerCallback};
use rsnano_core::{BlockHash, WalletId};
use rsnano_node::wallets::Wallets;
use rsnano_store_lmdb::Wallet;
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr},
    sync::{Arc, MutexGuard},
};

pub struct LmdbWalletsHandle(Wallets);

#[no_mangle]
pub extern "C" fn rsn_lmdb_wallets_create(
    enable_voting: bool,
    lmdb: &LmdbEnvHandle,
) -> *mut LmdbWalletsHandle {
    Box::into_raw(Box::new(LmdbWalletsHandle(Wallets::new(
        enable_voting,
        Arc::clone(lmdb),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_destroy(handle: *mut LmdbWalletsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_init(
    handle: *mut LmdbWalletsHandle,
    txn: *mut TransactionHandle,
) -> bool {
    (*handle).0.initialize((*txn).as_write_txn()).is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_get_wallet_ids(
    handle: *mut LmdbWalletsHandle,
    txn: *mut TransactionHandle,
    result: *mut U256ArrayDto,
) {
    let wallet_ids = (*handle).0.get_wallet_ids((*txn).as_txn());
    let data = wallet_ids.iter().map(|i| *i.as_bytes()).collect();
    (*result).initialize(data)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_get_block_hash(
    handle: *mut LmdbWalletsHandle,
    txn: *mut TransactionHandle,
    id: *const c_char,
    hash: *mut u8,
) -> bool {
    let id = CStr::from_ptr(id).to_str().unwrap();
    match (*handle).0.get_block_hash((*txn).as_txn(), id) {
        Ok(Some(h)) => {
            h.copy_bytes(hash);
            true
        }
        Ok(None) => {
            BlockHash::zero().copy_bytes(hash);
            true
        }
        Err(_) => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_set_block_hash(
    handle: *mut LmdbWalletsHandle,
    txn: *mut TransactionHandle,
    id: *const c_char,
    hash: *const u8,
) -> bool {
    let id = CStr::from_ptr(id).to_str().unwrap();
    (*handle)
        .0
        .set_block_hash((*txn).as_write_txn(), id, &BlockHash::from_ptr(hash))
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_clear_send_ids(
    handle: *mut LmdbWalletsHandle,
    txn: *mut TransactionHandle,
) {
    (*handle).0.clear_send_ids((*txn).as_write_txn())
}

/// private key + raw_key
pub type RepresentativeCallback = unsafe extern "C" fn(*mut c_void, *const u8, *const u8);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_foreach_representative(
    handle: &LmdbWalletsHandle,
    callback: RepresentativeCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context = ContextWrapper::new(context, delete_context);
    handle.0.foreach_representative(move |account, raw_key| {
        let ctx = context.get_context();
        callback(ctx, &account.as_bytes()[0], &raw_key.as_bytes()[0]);
    });
}

pub struct WalletsMutexLockHandle(MutexGuard<'static, HashMap<WalletId, Arc<Wallet>>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_lock(
    handle: &LmdbWalletsHandle,
) -> *mut WalletsMutexLockHandle {
    let guard = unsafe {
        let guard = handle.0.mutex.lock().unwrap();
        std::mem::transmute::<
            MutexGuard<'_, HashMap<WalletId, Arc<Wallet>>>,
            MutexGuard<'static, HashMap<WalletId, Arc<Wallet>>>,
        >(guard)
    };
    Box::into_raw(Box::new(WalletsMutexLockHandle(guard)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_try_lock(
    handle: &LmdbWalletsHandle,
) -> *mut WalletsMutexLockHandle {
    match handle.0.mutex.try_lock() {
        Ok(guard) => {
            let guard = unsafe {
                std::mem::transmute::<
                    MutexGuard<'_, HashMap<WalletId, Arc<Wallet>>>,
                    MutexGuard<'static, HashMap<WalletId, Arc<Wallet>>>,
                >(guard)
            };
            Box::into_raw(Box::new(WalletsMutexLockHandle(guard)))
        }
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_lock_destroy(handle: *mut WalletsMutexLockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_lock_size(
    handle: &WalletsMutexLockHandle,
) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_lock_find(
    handle: &WalletsMutexLockHandle,
    wallet_id: *const u8,
    wallet: *mut *mut WalletHandle,
) -> bool {
    let wallet_id = WalletId::from_ptr(wallet_id);
    match handle.0.get(&wallet_id) {
        Some(w) => {
            *wallet = Box::into_raw(Box::new(WalletHandle(Arc::clone(w))));
            true
        }
        None => false,
    }
}

#[no_mangle]
pub extern "C" fn rsn_lmdb_wallets_mutex_lock_get_all(
    handle: &WalletsMutexLockHandle,
) -> *mut WalletVecHandle {
    let wallets = handle
        .0
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Box::into_raw(Box::new(WalletVecHandle(wallets)))
}

pub struct WalletVecHandle(Vec<(WalletId, Arc<Wallet>)>);

#[no_mangle]
pub extern "C" fn rsn_wallet_vec_len(handle: &WalletVecHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_vec_get(
    handle: &WalletVecHandle,
    index: usize,
    wallet_id: *mut u8,
) -> *mut WalletHandle {
    let (id, wallet) = handle.0.get(index).unwrap();
    id.copy_bytes(wallet_id);
    Box::into_raw(Box::new(WalletHandle(Arc::clone(wallet))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_vec_destroy(handle: *mut WalletVecHandle) {
    drop(Box::from_raw(handle));
}
