use super::wallet::WalletHandle;
use crate::{utils::ContextWrapper, VoidPointerCallback};
use rsnano_core::Amount;
use rsnano_node::wallets::WalletActionThread;
use std::{ffi::c_void, sync::Arc};

pub struct WalletActionThreadHandle(WalletActionThread);

#[no_mangle]
pub extern "C" fn rsn_wallet_action_thread_create() -> *mut WalletActionThreadHandle {
    Box::into_raw(Box::new(
        WalletActionThreadHandle(WalletActionThread::new()),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_action_thread_destroy(handle: *mut WalletActionThreadHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_wallet_action_thread_start(handle: &WalletActionThreadHandle) {
    handle.0.start()
}

#[no_mangle]
pub extern "C" fn rsn_wallet_action_thread_stop(handle: &WalletActionThreadHandle) {
    handle.0.stop()
}

pub type WalletActionCallback = extern "C" fn(*mut c_void, *mut WalletHandle);

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_action_thread_queue_wallet_action(
    handle: &WalletActionThreadHandle,
    amount: *const u8,
    wallet: &WalletHandle,
    action: WalletActionCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let amount = Amount::from_ptr(amount);
    let context_wrapper = ContextWrapper::new(context, delete_context);
    let wrapped_action = Box::new(move |wallet| {
        let ctx = context_wrapper.get_context();
        action(ctx, Box::into_raw(Box::new(WalletHandle(wallet))))
    });

    handle
        .0
        .queue_wallet_action(amount, Arc::clone(&wallet.0), wrapped_action)
}

#[no_mangle]
pub extern "C" fn rsn_wallet_action_thread_len(handle: &WalletActionThreadHandle) -> usize {
    handle.0.len()
}

pub type WalletActionObserverCallback = extern "C" fn(*mut c_void, bool);

#[no_mangle]
pub extern "C" fn rsn_wallet_action_thread_set_observer(
    handle: &mut WalletActionThreadHandle,
    observer: WalletActionObserverCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(context, delete_context);
    let wrapped_observer = Box::new(move |active| {
        let ctx = context_wrapper.get_context();
        observer(ctx, active);
    });
    handle.0.set_observer(wrapped_observer);
}
