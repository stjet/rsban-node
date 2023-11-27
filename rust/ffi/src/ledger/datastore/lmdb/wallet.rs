use rsnano_store_lmdb::Wallet;
use std::sync::MutexGuard;

pub struct WalletHandle(Wallet);

#[no_mangle]
pub extern "C" fn rsn_wallet_create() -> *mut WalletHandle {
    Box::into_raw(Box::new(WalletHandle(Wallet::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_destroy(handle: *mut WalletHandle) {
    drop(Box::from_raw(handle))
}

pub struct RepresentativesLockHandle(MutexGuard<'static, ()>);

#[no_mangle]
pub extern "C" fn rsn_representatives_lock_create(
    handle: &WalletHandle,
) -> *mut RepresentativesLockHandle {
    let guard = handle.0.representatives.lock().unwrap();
    let guard = unsafe { std::mem::transmute::<MutexGuard<()>, MutexGuard<'static, ()>>(guard) };
    Box::into_raw(Box::new(RepresentativesLockHandle(guard)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representatives_lock_destroy(handle: *mut RepresentativesLockHandle) {
    drop(Box::from_raw(handle))
}
