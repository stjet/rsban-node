use std::{ffi::c_void, sync::Arc};

use crate::{
    datastore::lmdb::{LmdbReadTransaction, TxnCallbacks},
    ffi::VoidPointerCallback,
};

pub struct TransactionHandle(TransactionType);

enum TransactionType {
    Read(LmdbReadTransaction),
}

#[no_mangle]
pub extern "C" fn rsn_lmdb_read_txn_create(
    txn_id: u64,
    callbacks: *mut c_void,
) -> *mut TransactionHandle {
    let callbacks = Arc::new(FfiCallbacksWrapper::new(callbacks));
    Box::into_raw(Box::new(TransactionHandle(TransactionType::Read(
        LmdbReadTransaction::new(txn_id, callbacks),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_read_txn_destroy(handle: *mut TransactionHandle) {
    drop(Box::from_raw(handle))
}

struct FfiCallbacksWrapper {
    handle: *mut c_void,
}

impl FfiCallbacksWrapper {
    fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

impl TxnCallbacks for FfiCallbacksWrapper {
    fn txn_start(&self, txn_id: u64, is_write: bool) {
        unsafe { TXN_START.expect("TXN_START")(self.handle, txn_id, is_write) }
    }

    fn txn_end(&self, txn_id: u64) {
        unsafe { TXN_END.expect("TXN_END")(self.handle, txn_id) }
    }
}

impl Drop for FfiCallbacksWrapper {
    fn drop(&mut self) {
        unsafe { TXN_CALLBACKS_DESTROY.expect("TXN_CALLBACKS_DESTROY missing")(self.handle) }
    }
}

static mut TXN_CALLBACKS_DESTROY: Option<VoidPointerCallback> = None;
pub type TxnStartCallback = unsafe extern "C" fn(*mut c_void, u64, bool);
pub type TxnEndCallback = unsafe extern "C" fn(*mut c_void, u64);
static mut TXN_START: Option<TxnStartCallback> = None;
static mut TXN_END: Option<TxnEndCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_txn_callbacks_destroy(f: VoidPointerCallback) {
    TXN_CALLBACKS_DESTROY = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_txn_callbacks_start(f: TxnStartCallback) {
    TXN_START = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_txn_callbacks_end(f: TxnEndCallback) {
    TXN_END = Some(f);
}
