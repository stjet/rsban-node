mod account_store;
mod iterator;
mod lmdb_env;

use std::{
    ffi::{c_void, CStr, CString},
    os::raw::c_char,
    ptr,
    sync::Arc,
};

pub use account_store::LmdbAccountStore;
pub use iterator::{LmdbIterator, LmdbRawIterator};
pub use lmdb_env::LmdbEnv;

use crate::utils::{MemoryStream, Serialize};

use super::{ReadTransaction, Transaction, WriteTransaction};

pub struct LmdbReadTransaction {
    env: *mut MdbEnv,
    txn_id: u64,
    callbacks: Arc<dyn TxnCallbacks>,
    pub handle: *mut MdbTxn,
}

impl LmdbReadTransaction {
    pub unsafe fn new(txn_id: u64, env: *mut MdbEnv, callbacks: Arc<dyn TxnCallbacks>) -> Self {
        let mut handle: *mut MdbTxn = ptr::null_mut();
        let status = mdb_txn_begin(env, ptr::null_mut(), MDB_RDONLY, &mut handle);
        assert!(status == 0);
        callbacks.txn_start(txn_id, false);

        Self {
            env,
            txn_id,
            callbacks,
            handle,
        }
    }

    pub fn reset(&mut self) {
        unsafe { mdb_txn_reset(self.handle) };
        self.callbacks.txn_end(self.txn_id);
    }

    pub fn renew(&mut self) {
        let status = unsafe { mdb_txn_renew(self.handle) };
        assert!(status == 0);
        self.callbacks.txn_start(self.txn_id, false);
    }

    pub fn refresh(&mut self) {
        self.reset();
        self.renew();
    }
}

impl Drop for LmdbReadTransaction {
    fn drop(&mut self) {
        // This uses commit rather than abort, as it is needed when opening databases with a read only transaction
        let status = unsafe { mdb_txn_commit(self.handle) };
        assert!(status == MDB_SUCCESS);
        self.callbacks.txn_end(self.txn_id);
    }
}

impl Transaction for LmdbReadTransaction {
    fn as_any(&self) -> &(dyn std::any::Any + '_) {
        self
    }
}

impl ReadTransaction for LmdbReadTransaction {}

pub struct LmdbWriteTransaction {
    env: *mut MdbEnv,
    txn_id: u64,
    callbacks: Arc<dyn TxnCallbacks>,
    pub handle: *mut MdbTxn,
    active: bool,
}

impl LmdbWriteTransaction {
    pub fn new(txn_id: u64, env: *mut MdbEnv, callbacks: Arc<dyn TxnCallbacks>) -> Self {
        let mut tx = Self {
            env,
            txn_id,
            callbacks,
            handle: ptr::null_mut(),
            active: true,
        };
        tx.renew();
        tx
    }

    pub fn commit(&mut self) {
        if self.active {
            let status = unsafe { mdb_txn_commit(self.handle) };
            if status != MDB_SUCCESS {
                panic!("Unable to write to the LMDB database {}", unsafe {
                    mdb_strerror(status)
                });
            }
            self.callbacks.txn_end(self.txn_id);
            self.active = false;
        }
    }

    pub fn renew(&mut self) {
        let status = unsafe { mdb_txn_begin(self.env, ptr::null_mut(), 0, &mut self.handle) };
        if status != MDB_SUCCESS {
            panic!("write tx renew failed: {}", unsafe { mdb_strerror(status) });
        }
        self.callbacks.txn_start(self.txn_id, true);
        self.active = true;
    }

    pub fn refresh(&mut self) {
        self.commit();
        self.renew();
    }
}

impl Drop for LmdbWriteTransaction {
    fn drop(&mut self) {
        self.commit();
    }
}

impl Transaction for LmdbWriteTransaction {
    fn as_any(&self) -> &(dyn std::any::Any + '_) {
        self
    }
}

impl WriteTransaction for LmdbWriteTransaction {
    fn as_transaction(&self) -> &dyn Transaction {
        self
    }
}

pub trait TxnCallbacks {
    fn txn_start(&self, txn_id: u64, is_write: bool);
    fn txn_end(&self, txn_id: u64);
}

pub fn assert_success(status: i32) {
    if status != MDB_SUCCESS {
        let msg = unsafe { mdb_strerror(status) };
        panic!("LMDB status: {}", msg);
    }
}

#[repr(C)]
#[derive(PartialEq, Eq)]
pub enum MdbCursorOp {
    MdbFirst,        // Position at first key/data item */
    MdbFirstDup,     // Position at first data item of current key.  Only for #MDB_DUPSORT */
    MdbGetBoth,      // Position at key/data pair. Only for #MDB_DUPSORT */
    MdbGetBothRange, // position at key, nearest data. Only for #MDB_DUPSORT */
    MdbGetCurrent,   // Return key/data at current cursor position */
    MdbGetMultiple, // Return up to a page of duplicate data items from current cursor position. Move cursor to prepare for #MDB_NEXT_MULTIPLE. Only for #MDB_DUPFIXED */
    MdbLast,        // Position at last key/data item */
    MdbLastDup,     // Position at last data item of current key.  Only for #MDB_DUPSORT */
    MdbNext,        // Position at next data item */
    MdbNextDup,     // Position at next data item of current key.  Only for #MDB_DUPSORT */
    MdbNextMultiple, // Return up to a page of duplicate data items from next cursor position. Move cursor to prepare for #MDB_NEXT_MULTIPLE. Only for #MDB_DUPFIXED */
    MdbNextNodup,    // Position at first data item of next key */
    MdbPrev,         // Position at previous data item */
    MdbPrevDup,      // Position at previous data item of current key.  Only for #MDB_DUPSORT */
    MdbPrevNodup,    // Position at last data item of previous key */
    MdbSet,          // Position at specified key */
    MdbSetKey,       // Position at specified key, return key + data */
    MdbSetRange,     // Position at first key greater than or equal to specified key. */
    MdbPrevMultiple, // Position at previous page and return up to a page of duplicate data items. Only for #MDB_DUPFIXED */
}

#[repr(C)]
#[derive(Clone)]
pub struct MdbVal {
    pub mv_size: usize,       // size of the data item
    pub mv_data: *mut c_void, // address of the data item
}

impl MdbVal {
    pub fn new() -> Self {
        Self {
            mv_size: 0,
            mv_data: ptr::null_mut(),
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.mv_data as *const u8, self.mv_size) }
    }
}

impl Default for MdbVal {
    fn default() -> Self {
        Self::new()
    }
}

pub struct OwnedMdbVal {
    bytes: Vec<u8>,
    val: MdbVal,
}

impl OwnedMdbVal {
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            val: MdbVal {
                mv_size: 0,
                mv_data: ptr::null_mut(),
            },
        }
    }
    pub fn as_mdb_val(&mut self) -> &mut MdbVal {
        self.val.mv_size = self.bytes.len();
        self.val.mv_data = self.bytes.as_mut_ptr() as *mut c_void;
        &mut self.val
    }
}

impl<T> From<&T> for OwnedMdbVal
where
    T: Serialize,
{
    fn from(value: &T) -> Self {
        let mut stream = MemoryStream::new();
        value.serialize(&mut stream).unwrap();
        OwnedMdbVal::new(stream.to_vec())
    }
}

pub fn get_raw_lmdb_txn(txn: &dyn Transaction) -> *mut MdbTxn {
    let any = txn.as_any();
    if let Some(t) = any.downcast_ref::<LmdbReadTransaction>() {
        t.handle
    } else if let Some(t) = any.downcast_ref::<LmdbWriteTransaction>() {
        t.handle
    } else {
        panic!("not an LMDB transaction");
    }
}

#[repr(C)]
pub struct MdbEnv {}

#[repr(C)]
pub struct MdbTxn {}

#[repr(C)]
pub struct MdbCursor {}

pub type MdbTxnBeginCallback =
    extern "C" fn(*mut MdbEnv, *mut MdbTxn, u32, *mut *mut MdbTxn) -> i32;
pub type MdbTxnCommitCallback = extern "C" fn(*mut MdbTxn) -> i32;
pub type MdbTxnResetCallback = extern "C" fn(*mut MdbTxn);
pub type MdbTxnRenewCallback = extern "C" fn(*mut MdbTxn) -> i32;
pub type MdbStrerrorCallback = extern "C" fn(i32) -> *mut c_char;
pub type MdbCursorOpenCallback = extern "C" fn(*mut MdbTxn, u32, *mut *mut MdbCursor) -> i32;
pub type MdbCursorGetCallback =
    extern "C" fn(*mut MdbCursor, *mut MdbVal, *mut MdbVal, MdbCursorOp) -> i32;
pub type MdbCursorCloseCallback = extern "C" fn(*mut MdbCursor);
pub type MdbDbiOpenCallback = extern "C" fn(*mut MdbTxn, *const i8, u32, *mut u32) -> i32;
pub type MdbPutCallback = extern "C" fn(*mut MdbTxn, u32, *mut MdbVal, *mut MdbVal, u32) -> i32;
pub type MdbGetCallback = extern "C" fn(*mut MdbTxn, u32, *mut MdbVal, *mut MdbVal) -> i32;
pub type MdbDelCallback = extern "C" fn(*mut MdbTxn, u32, *mut MdbVal, *mut MdbVal) -> i32;

pub static mut MDB_TXN_BEGIN: Option<MdbTxnBeginCallback> = None;
pub static mut MDB_TXN_COMMIT: Option<MdbTxnCommitCallback> = None;
pub static mut MDB_TXN_RESET: Option<MdbTxnResetCallback> = None;
pub static mut MDB_TXN_RENEW: Option<MdbTxnRenewCallback> = None;
pub static mut MDB_STRERROR: Option<MdbStrerrorCallback> = None;
pub static mut MDB_CURSOR_OPEN: Option<MdbCursorOpenCallback> = None;
pub static mut MDB_CURSOR_GET: Option<MdbCursorGetCallback> = None;
pub static mut MDB_CURSOR_CLOSE: Option<MdbCursorCloseCallback> = None;
pub static mut MDB_DBI_OPEN: Option<MdbDbiOpenCallback> = None;
pub static mut MDB_PUT: Option<MdbPutCallback> = None;
pub static mut MDB_GET: Option<MdbGetCallback> = None;
pub static mut MDB_DEL: Option<MdbDelCallback> = None;

pub unsafe fn mdb_txn_begin(
    env: *mut MdbEnv,
    parent: *mut MdbTxn,
    flags: u32,
    result: *mut *mut MdbTxn,
) -> i32 {
    MDB_TXN_BEGIN.expect("MDB_TXN_BEGIN missing")(env, parent, flags, result)
}

pub unsafe fn mdb_txn_commit(txn: *mut MdbTxn) -> i32 {
    MDB_TXN_COMMIT.expect("MDB_TXN_COMMIT missing")(txn)
}

pub unsafe fn mdb_txn_reset(txn: *mut MdbTxn) {
    MDB_TXN_RESET.expect("MDB_TXN_RESET missing")(txn)
}

pub unsafe fn mdb_txn_renew(txn: *mut MdbTxn) -> i32 {
    MDB_TXN_RENEW.expect("MDB_TXN_RENEW missing")(txn)
}

pub unsafe fn mdb_strerror(status: i32) -> &'static str {
    let ptr = MDB_STRERROR.expect("MDB_STRERROR missing")(status);
    CStr::from_ptr(ptr).to_str().unwrap()
}

pub unsafe fn mdb_cursor_open(txn: *mut MdbTxn, dbi: u32, cursor: *mut *mut MdbCursor) -> i32 {
    MDB_CURSOR_OPEN.expect("MDB_CURSOR_OPEN missing")(txn, dbi, cursor)
}

pub unsafe fn mdb_cursor_get(
    cursor: *mut MdbCursor,
    key: &mut MdbVal,
    value: &mut MdbVal,
    op: MdbCursorOp,
) -> i32 {
    MDB_CURSOR_GET.expect("MDB_CURSOR_GET missing")(cursor, key, value, op)
}

pub unsafe fn mdb_cursor_close(cursor: *mut MdbCursor) {
    MDB_CURSOR_CLOSE.expect("MDB_CURSOR_CLOSE missing")(cursor);
}

pub unsafe fn mdb_dbi_open(txn: *mut MdbTxn, name: &str, flags: u32, dbi: &mut u32) -> i32 {
    let name_cstr = CString::new(name).unwrap();
    MDB_DBI_OPEN.expect("MDB_DBI_OPEN missing")(txn, name_cstr.as_ptr(), flags, dbi)
}

pub unsafe fn mdb_put(
    txn: *mut MdbTxn,
    dbi: u32,
    key: &mut MdbVal,
    data: &mut MdbVal,
    flags: u32,
) -> i32 {
    MDB_PUT.expect("MDB_PUT missing")(txn, dbi, key, data, flags)
}

pub unsafe fn mdb_get(txn: *mut MdbTxn, dbi: u32, key: &mut MdbVal, data: &mut MdbVal) -> i32 {
    MDB_GET.expect("MDB_GET missing")(txn, dbi, key, data)
}

pub unsafe fn mdb_del(
    txn: *mut MdbTxn,
    dbi: u32,
    key: &mut MdbVal,
    data: Option<&mut MdbVal>,
) -> i32 {
    let dataptr = data.map(|v| v as *mut MdbVal).unwrap_or(ptr::null_mut());
    MDB_DEL.expect("MDB_DEL missing")(txn, dbi, key, dataptr)
}

/// Successful result
const MDB_SUCCESS: i32 = 0;

/// read only
const MDB_RDONLY: u32 = 0x20000;

/// key/data pair not found (EOF)
const MDB_NOTFOUND: i32 = -30798;
