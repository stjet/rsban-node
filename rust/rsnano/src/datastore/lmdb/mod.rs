mod iterator;

use std::{
    ffi::{c_void, CStr},
    os::raw::c_char,
    ptr,
    sync::Arc,
};

pub use iterator::LmdbIterator;

use super::{ReadTransaction, Transaction};

pub struct LmdbReadTransaction {
    env: *mut c_void,
    txn_id: u64,
    callbacks: Arc<dyn TxnCallbacks>,
    pub handle: *mut c_void,
}

impl LmdbReadTransaction {
    pub fn new(txn_id: u64, env: *mut c_void, callbacks: Arc<dyn TxnCallbacks>) -> Self {
        let mut handle: *mut c_void = ptr::null_mut();
        let status = unsafe { mdb_txn_begin(env, ptr::null_mut(), MDB_RDONLY, &mut handle) };
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

impl Transaction for LmdbReadTransaction {}

impl ReadTransaction for LmdbReadTransaction {}

pub struct LmdbWriteTransaction {
    env: *mut c_void,
    txn_id: u64,
    callbacks: Arc<dyn TxnCallbacks>,
    pub handle: *mut c_void,
    active: bool,
}

impl LmdbWriteTransaction {
    pub fn new(txn_id: u64, env: *mut c_void, callbacks: Arc<dyn TxnCallbacks>) -> Self {
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

impl Transaction for LmdbWriteTransaction {}

pub trait TxnCallbacks {
    fn txn_start(&self, txn_id: u64, is_write: bool);
    fn txn_end(&self, txn_id: u64);
}

/// args: MDB_env* env, MDB_txn* parent, flags, MDB_txn** ret
pub type MdbTxnBeginCallback =
    extern "C" fn(*mut c_void, *mut c_void, u32, *mut *mut c_void) -> i32;

/// args: MDB_txn*
pub type MdbTxnCommitCallback = extern "C" fn(*mut c_void) -> i32;

/// args: MDB_txn*
pub type MdbTxnResetCallback = extern "C" fn(*mut c_void);

/// args: MDB_txn*
pub type MdbTxnRenewCallback = extern "C" fn(*mut c_void) -> i32;

/// args: status
pub type MdbStrerrorCallback = extern "C" fn(i32) -> *mut c_char;

///args: MDB_txn*, MDB_dbi, MDB_cursor**
pub type MdbCursorOpenCallback = extern "C" fn(*mut c_void, u32, *mut *mut c_void) -> i32;

pub static mut MDB_TXN_BEGIN: Option<MdbTxnBeginCallback> = None;
pub static mut MDB_TXN_COMMIT: Option<MdbTxnCommitCallback> = None;
pub static mut MDB_TXN_RESET: Option<MdbTxnResetCallback> = None;
pub static mut MDB_TXN_RENEW: Option<MdbTxnRenewCallback> = None;
pub static mut MDB_STRERROR: Option<MdbStrerrorCallback> = None;
pub static mut MDB_CURSOR_OPEN: Option<MdbCursorOpenCallback> = None;

pub unsafe fn mdb_txn_begin(
    env: *mut c_void,
    parent: *mut c_void,
    flags: u32,
    result: *mut *mut c_void,
) -> i32 {
    MDB_TXN_BEGIN.expect("MDB_TXN_BEGIN missing")(env, parent, flags, result)
}

pub unsafe fn mdb_txn_commit(txn: *mut c_void) -> i32 {
    MDB_TXN_COMMIT.expect("MDB_TXN_COMMIT missing")(txn)
}

pub unsafe fn mdb_txn_reset(txn: *mut c_void) {
    MDB_TXN_RESET.expect("MDB_TXN_RESET missing")(txn)
}

pub unsafe fn mdb_txn_renew(txn: *mut c_void) -> i32 {
    MDB_TXN_RENEW.expect("MDB_TXN_RENEW missing")(txn)
}

pub unsafe fn mdb_strerror(status: i32) -> &'static str {
    let ptr = MDB_STRERROR.expect("MDB_STRERROR missing")(status);
    CStr::from_ptr(ptr).to_str().unwrap()
}

pub unsafe fn mdb_cursor_open(txn: *mut c_void, dbi: u32, cursor: *mut *mut c_void) -> i32 {
    MDB_CURSOR_OPEN.expect("MDB_CURSOR_OPEN missing")(txn, dbi, cursor)
}

///	Successful result
const MDB_SUCCESS: i32 = 0;

/// read only
const MDB_RDONLY: u32 = 0x20000;
