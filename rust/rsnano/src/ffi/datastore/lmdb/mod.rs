mod account_store;
mod iterator;

use std::{ffi::c_void, ops::Deref, sync::Arc};

use crate::{
    datastore::{
        lmdb::{
            LmdbReadTransaction, LmdbWriteTransaction, MdbCursorCloseCallback,
            MdbCursorGetCallback, MdbCursorOpenCallback, MdbDbiOpenCallback, MdbEnv,
            MdbStrerrorCallback, MdbTxn, MdbTxnBeginCallback, MdbTxnCommitCallback,
            MdbTxnRenewCallback, MdbTxnResetCallback, TxnCallbacks, MDB_CURSOR_CLOSE,
            MDB_CURSOR_GET, MDB_CURSOR_OPEN, MDB_DBI_OPEN, MDB_STRERROR, MDB_TXN_BEGIN,
            MDB_TXN_COMMIT, MDB_TXN_RENEW, MDB_TXN_RESET,
        },
        Transaction,
    },
    ffi::VoidPointerCallback,
};

pub struct TransactionHandle(TransactionType);

impl TransactionHandle {
    pub fn as_read_tx(&mut self) -> &mut LmdbReadTransaction {
        match &mut self.0 {
            TransactionType::Read(tx) => tx,
            _ => panic!("invalid tx type"),
        }
    }

    pub fn as_write_tx(&mut self) -> &mut LmdbWriteTransaction {
        match &mut self.0 {
            TransactionType::Write(tx) => tx,
            _ => panic!("invalid tx type"),
        }
    }

    pub fn as_txn(&self) -> &dyn Transaction {
        match &self.0 {
            TransactionType::Read(t) => t,
            TransactionType::Write(t) => t,
        }
    }
}

impl Deref for TransactionHandle {
    type Target = TransactionType;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub enum TransactionType {
    Read(LmdbReadTransaction),
    Write(LmdbWriteTransaction),
}

#[no_mangle]
pub extern "C" fn rsn_lmdb_read_txn_create(
    txn_id: u64,
    env: *mut MdbEnv,
    callbacks: *mut c_void,
) -> *mut TransactionHandle {
    let callbacks = Arc::new(FfiCallbacksWrapper::new(callbacks));
    Box::into_raw(Box::new(TransactionHandle(TransactionType::Read(
        LmdbReadTransaction::new(txn_id, env, callbacks),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_read_txn_destroy(handle: *mut TransactionHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_read_txn_reset(handle: *mut TransactionHandle) {
    (*handle).as_read_tx().reset();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_read_txn_renew(handle: *mut TransactionHandle) {
    (*handle).as_read_tx().renew();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_read_txn_refresh(handle: *mut TransactionHandle) {
    (*handle).as_read_tx().refresh();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_read_txn_handle(handle: *mut TransactionHandle) -> *mut MdbTxn {
    (*handle).as_read_tx().handle
}

#[no_mangle]
pub extern "C" fn rsn_lmdb_write_txn_create(
    txn_id: u64,
    env: *mut MdbEnv,
    callbacks: *mut c_void,
) -> *mut TransactionHandle {
    let callbacks = Arc::new(FfiCallbacksWrapper::new(callbacks));
    Box::into_raw(Box::new(TransactionHandle(TransactionType::Write(
        LmdbWriteTransaction::new(txn_id, env, callbacks),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_write_txn_destroy(handle: *mut TransactionHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_write_txn_commit(handle: *mut TransactionHandle) {
    (*handle).as_write_tx().commit();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_write_txn_renew(handle: *mut TransactionHandle) {
    (*handle).as_write_tx().renew();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_write_txn_refresh(handle: *mut TransactionHandle) {
    (*handle).as_write_tx().refresh();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_write_txn_handle(handle: *mut TransactionHandle) -> *mut MdbTxn {
    (*handle).as_write_tx().handle
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

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_mdb_txn_begin(f: MdbTxnBeginCallback) {
    MDB_TXN_BEGIN = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_mdb_txn_commit(f: MdbTxnCommitCallback) {
    MDB_TXN_COMMIT = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_mdb_txn_reset(f: MdbTxnResetCallback) {
    MDB_TXN_RESET = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_mdb_txn_renew(f: MdbTxnRenewCallback) {
    MDB_TXN_RENEW = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_mdb_strerror(f: MdbStrerrorCallback) {
    MDB_STRERROR = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_mdb_cursor_open(f: MdbCursorOpenCallback) {
    MDB_CURSOR_OPEN = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_mdb_cursor_get(f: MdbCursorGetCallback) {
    MDB_CURSOR_GET = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_mdb_cursor_close(f: MdbCursorCloseCallback) {
    MDB_CURSOR_CLOSE = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_mdb_dbi_open(f: MdbDbiOpenCallback) {
    MDB_DBI_OPEN = Some(f);
}
