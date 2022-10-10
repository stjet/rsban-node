use std::ffi::c_uint;

use lmdb::{Cursor, Database, RoCursor};
use lmdb_sys::{MDB_FIRST, MDB_LAST, MDB_NEXT, MDB_SET_RANGE};

use crate::datastore::iterator::DbIteratorImpl;

use super::LmdbTransaction;

pub struct LmdbIteratorImpl {
    current: Option<(&'static [u8], &'static [u8])>,
    cursor: Option<RoCursor<'static>>,
}

impl LmdbIteratorImpl {
    pub fn new(
        txn: &LmdbTransaction,
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
        //todo: dont use unsafe code:
        let cursor =
            unsafe { std::mem::transmute::<lmdb::RoCursor<'_>, lmdb::RoCursor<'static>>(cursor) };
        let mut result = Self {
            current: None,
            cursor: Some(cursor),
        };
        result.load_current(key_val, operation);
        result
    }

    fn load_current(&mut self, key: Option<&[u8]>, operation: c_uint) {
        let result = self.cursor.as_ref().unwrap().get(key, None, operation);
        self.current = match result {
            Err(lmdb::Error::NotFound) => None,
            Ok((Some(k), v)) => Some((k, v)),
            Ok(_) => unreachable!(),
            Err(_) => {
                result.unwrap();
                unreachable!()
            }
        };
    }

    pub fn null() -> Self {
        Self {
            current: None,
            cursor: None,
        }
    }
}

impl DbIteratorImpl for LmdbIteratorImpl {
    fn current(&self) -> Option<(&[u8], &[u8])> {
        self.current
    }

    fn next(&mut self) {
        self.load_current(None, MDB_NEXT);
    }
}
