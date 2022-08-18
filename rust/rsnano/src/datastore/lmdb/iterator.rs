use super::{mdb_cursor_close, mdb_cursor_open, MdbCursor, MdbTxn, MdbVal};
use crate::datastore::lmdb::{mdb_cursor_get, MdbCursorOp, MDB_NOTFOUND, MDB_SUCCESS};
use std::ptr;

pub struct LmdbIterator {
    cursor: *mut MdbCursor,
    pub key: MdbVal,
    pub value: MdbVal,
    expected_key_size: usize,
}

impl LmdbIterator {
    pub fn new(
        txn: *mut MdbTxn,
        dbi: u32,
        val: &MdbVal,
        direction_asc: bool,
        expected_key_size: usize,
    ) -> Self {
        let mut iterator = Self {
            cursor: ptr::null_mut(),
            key: MdbVal::new(),
            value: MdbVal::new(),
            expected_key_size,
        };
        iterator.init(txn, dbi, val, direction_asc);
        iterator
    }

    fn init(&mut self, txn: *mut MdbTxn, dbi: u32, val_a: &MdbVal, direction_asc: bool) {
        let status = unsafe { mdb_cursor_open(txn, dbi, &mut self.cursor) };
        assert!(status == MDB_SUCCESS);

        let mut operation = MdbCursorOp::MdbSetRange;
        if val_a.mv_size != 0 {
            self.key = val_a.clone();
        } else {
            operation = if direction_asc {
                MdbCursorOp::MdbFirst
            } else {
                MdbCursorOp::MdbLast
            };
        }
        let status2 =
            unsafe { mdb_cursor_get(self.cursor, &mut self.key, &mut self.value, operation) };
        assert!(status2 == MDB_SUCCESS || status2 == MDB_NOTFOUND);
        if status2 != MDB_NOTFOUND {
            let status3 = unsafe {
                mdb_cursor_get(
                    self.cursor,
                    &mut self.key,
                    &mut self.value,
                    MdbCursorOp::MdbGetCurrent,
                )
            };
            assert!(status3 == MDB_SUCCESS || status3 == MDB_NOTFOUND);
            if self.key.mv_size != self.expected_key_size {
                self.clear();
            }
        } else {
            self.clear();
        }
    }

    pub fn clear(&mut self) {
        self.key = MdbVal::new();
        self.value = MdbVal::new();
    }

    pub fn cursor(&self) -> *mut MdbCursor {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: *mut MdbCursor) {
        self.cursor = cursor;
    }

    pub fn next(&mut self) {
        debug_assert!(!self.cursor.is_null());
        let status = unsafe {
            mdb_cursor_get(
                self.cursor,
                &mut self.key,
                &mut self.value,
                MdbCursorOp::MdbNext,
            )
        };
        assert!(status == MDB_SUCCESS || status == MDB_NOTFOUND);
        if status == MDB_NOTFOUND {
            self.clear();
        }
        if self.key.mv_size != self.expected_key_size {
            self.clear();
        }
    }

    pub fn previous(&mut self) {
        debug_assert!(!self.cursor.is_null());
        let status = unsafe {
            mdb_cursor_get(
                self.cursor,
                &mut self.key,
                &mut self.value,
                MdbCursorOp::MdbPrev,
            )
        };
        assert!(status == MDB_SUCCESS || status == MDB_NOTFOUND);
        if status == MDB_NOTFOUND {
            self.clear();
        }
        if self.key.mv_size != self.expected_key_size {
            self.clear();
        }
    }
}

impl Drop for LmdbIterator {
    fn drop(&mut self) {
        if !self.cursor.is_null() {
            unsafe { mdb_cursor_close(self.cursor) };
        }
    }
}
