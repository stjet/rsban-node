use std::{ffi::c_void, ptr, sync::Arc};

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

pub static mut MDB_TXN_BEGIN: Option<MdbTxnBeginCallback> = None;
pub static mut MDB_TXN_COMMIT: Option<MdbTxnCommitCallback> = None;
pub static mut MDB_TXN_RESET: Option<MdbTxnResetCallback> = None;
pub static mut MDB_TXN_RENEW: Option<MdbTxnRenewCallback> = None;

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

///	Successful result
const MDB_SUCCESS: i32 = 0;

/// read only
const MDB_RDONLY: u32 = 0x20000;
