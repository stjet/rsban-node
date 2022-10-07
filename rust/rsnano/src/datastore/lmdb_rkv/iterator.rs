use std::ffi::c_uint;

use lmdb::{Cursor, Database, RoCursor};
use lmdb_sys::{MDB_FIRST, MDB_LAST, MDB_NEXT, MDB_SET_RANGE};

use crate::datastore::iterator::DbIteratorImpl;

use super::LmdbTransaction;

pub struct LmdbIteratorImpl<'a> {
    current: Option<(&'a [u8], &'a [u8])>,
    cursor: Option<RoCursor<'a>>,
}

impl<'a> LmdbIteratorImpl<'a> {
    pub fn new(
        txn: &'a LmdbTransaction<'a>,
        dbi: Database,
        key_val: Option<&[u8]>,
        direction_asc: bool,
    ) -> Self {
        let operation = if key_val.is_some() {
            MDB_SET_RANGE
        } else {
            if direction_asc {
                MDB_FIRST
            } else {
                MDB_LAST
            }
        };

        let cursor = txn.open_ro_cursor(dbi).unwrap();
        let mut result = Self {
            current: None,
            cursor: Some(cursor),
        };
        result.load_current(key_val, operation);
        result
    }

    fn load_current(&mut self, key: Option<&[u8]>, operation: c_uint) {
        let (k, v) = self
            .cursor
            .as_ref()
            .unwrap()
            .get(key, None, operation)
            .unwrap();
        self.current = match k {
            Some(bytes) => Some((bytes, v)),
            None => None,
        };
    }

    pub fn null() -> Self {
        Self {
            current: None,
            cursor: None,
        }
    }
}

impl<'a> DbIteratorImpl for LmdbIteratorImpl<'a> {
    fn current(&self) -> Option<(&'a [u8], &'a [u8])> {
        self.current
    }

    fn next(&mut self) {
        self.load_current(None, MDB_NEXT);
    }
}
