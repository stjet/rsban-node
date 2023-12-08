use rsnano_core::Account;
use rsnano_node::wallets::Wallet;
use std::{
    collections::HashSet,
    sync::{Arc, MutexGuard},
};

pub struct WalletHandle(pub Arc<Wallet>);

#[no_mangle]
pub extern "C" fn rsn_wallet_create() -> *mut WalletHandle {
    Box::into_raw(Box::new(WalletHandle(Arc::new(Wallet::new()))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_destroy(handle: *mut WalletHandle) {
    drop(Box::from_raw(handle))
}

pub struct RepresentativesLockHandle(MutexGuard<'static, HashSet<Account>>);

#[no_mangle]
pub extern "C" fn rsn_representatives_lock_create(
    handle: &WalletHandle,
) -> *mut RepresentativesLockHandle {
    let guard = handle.0.representatives.lock().unwrap();
    let guard = unsafe {
        std::mem::transmute::<MutexGuard<HashSet<Account>>, MutexGuard<'static, HashSet<Account>>>(
            guard,
        )
    };
    Box::into_raw(Box::new(RepresentativesLockHandle(guard)))
}

#[no_mangle]
pub extern "C" fn rsn_representatives_lock_size(handle: &RepresentativesLockHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representatives_lock_insert(
    handle: &mut RepresentativesLockHandle,
    rep: *const u8,
) {
    let rep = Account::from_ptr(rep);
    handle.0.insert(rep);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representatives_lock_get_all(
    handle: &mut RepresentativesLockHandle,
) -> *mut AccountVecHandle {
    let accounts: Vec<_> = handle.0.iter().cloned().collect();
    Box::into_raw(Box::new(AccountVecHandle(accounts)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representatives_lock_clear(handle: &mut RepresentativesLockHandle) {
    handle.0.clear();
}

pub struct AccountVecHandle(Vec<Account>);

#[no_mangle]
pub unsafe extern "C" fn rsn_account_vec_destroy(handle: *mut AccountVecHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_account_vec_len(handle: &AccountVecHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_account_vec_get(
    handle: &AccountVecHandle,
    index: usize,
    result: *mut u8,
) {
    handle.0[index].copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representatives_lock_destroy(handle: *mut RepresentativesLockHandle) {
    drop(Box::from_raw(handle))
}
