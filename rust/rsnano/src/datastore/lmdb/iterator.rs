use super::mdb_cursor_open;
use crate::datastore::lmdb::MDB_SUCCESS;
use std::{ffi::c_void, ptr};

pub struct LmdbIterator {
    cursor: *mut c_void, //a MDB_cursor
}

impl LmdbIterator {
    pub fn new(txn: *mut c_void, dbi: u32) -> Self {
        let mut cursor = ptr::null_mut();
        let status = unsafe { mdb_cursor_open(txn, dbi, &mut cursor) };
        assert!(status == MDB_SUCCESS);

        Self { cursor }
    }

    pub fn cursor(&self) -> *mut c_void {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: *mut c_void) {
        self.cursor = cursor;
    }
}
