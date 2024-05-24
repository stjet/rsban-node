use super::ConfiguredDatabase;
use lmdb_sys::{MDB_FIRST, MDB_LAST, MDB_NEXT, MDB_SET_RANGE};
use std::cell::Cell;

pub struct RoCursor(RoCursorStrategy);

impl RoCursor {
    pub fn new_null(database: ConfiguredDatabase) -> Self {
        Self(RoCursorStrategy::Nulled(RoCursorStub {
            database,
            current: Cell::new(0),
            ascending: Cell::new(true),
        }))
    }

    pub fn new(cursor: lmdb::RoCursor<'static>) -> Self {
        Self(RoCursorStrategy::Real(RoCursorWrapper(cursor)))
    }

    pub fn iter_start(
        &mut self,
    ) -> impl Iterator<Item = lmdb::Result<(&'static [u8], &'static [u8])>> {
        match &mut self.0 {
            RoCursorStrategy::Real(s) => s.iter_start(),
            RoCursorStrategy::Nulled(_) => todo!(),
        }
    }

    pub fn get(
        &self,
        key: Option<&[u8]>,
        data: Option<&[u8]>,
        op: u32,
    ) -> lmdb::Result<(Option<&'static [u8]>, &'static [u8])> {
        match &self.0 {
            RoCursorStrategy::Real(s) => s.get(key, data, op),
            RoCursorStrategy::Nulled(s) => s.get(key, data, op),
        }
    }
}

enum RoCursorStrategy {
    Real(RoCursorWrapper),
    Nulled(RoCursorStub),
}

//todo don't use static lifetimes!
struct RoCursorWrapper(lmdb::RoCursor<'static>);

impl RoCursorWrapper {
    fn iter_start(&mut self) -> lmdb::Iter<'static> {
        lmdb::Cursor::iter_start(&mut self.0)
    }

    fn get(
        &self,
        key: Option<&[u8]>,
        data: Option<&[u8]>,
        op: u32,
    ) -> lmdb::Result<(Option<&'static [u8]>, &'static [u8])> {
        lmdb::Cursor::get(&self.0, key, data, op)
    }
}

struct RoCursorStub {
    database: ConfiguredDatabase,
    current: Cell<i32>,
    ascending: Cell<bool>,
}

impl RoCursorStub {
    fn get(
        &self,
        key: Option<&[u8]>,
        _data: Option<&[u8]>,
        op: u32,
    ) -> lmdb::Result<(Option<&'static [u8]>, &'static [u8])> {
        if op == MDB_FIRST {
            self.current.set(0);
            self.ascending.set(true);
        } else if op == MDB_LAST {
            let entry_count = self.database.entries.len();
            self.ascending.set(false);
            self.current.set((entry_count as i32) - 1);
        } else if op == MDB_NEXT {
            if self.ascending.get() {
                self.current.set(self.current.get() + 1);
            } else {
                self.current.set(self.current.get() - 1);
            }
        } else if op == MDB_SET_RANGE {
            self.current.set(
                self.database
                    .entries
                    .keys()
                    .enumerate()
                    .find_map(|(i, k)| {
                        if Some(k.as_slice()) >= key {
                            Some(i as i32)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(i32::MAX),
            );
        } else {
            unimplemented!()
        }

        let current = self.current.get();
        if current < 0 {
            return Err(lmdb::Error::NotFound);
        }

        self.database
            .entries
            .iter()
            .nth(current as usize)
            .map(|(k, v)| unsafe {
                (
                    Some(std::mem::transmute::<&'_ [u8], &'static [u8]>(k.as_slice())),
                    std::mem::transmute::<&'_ [u8], &'static [u8]>(v.as_slice()),
                )
            })
            .ok_or(lmdb::Error::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LmdbDatabase, LmdbEnv};

    #[test]
    fn nulled_cursor_can_be_iterated_backwards() {
        let env = LmdbEnv::new_null_with()
            .database("foo", LmdbDatabase::new_null(42))
            .entry(&[1, 2, 3], &[4, 5, 6])
            .entry(&[2, 2, 2], &[6, 6, 6])
            .build()
            .build();

        let txn = env.tx_begin_read();

        let cursor = txn
            .txn()
            .open_ro_cursor(LmdbDatabase::new_null(42))
            .unwrap();
        let result = cursor.get(None, None, MDB_LAST);
        assert_eq!(
            result,
            Ok((Some([2u8, 2, 2].as_slice()), [6u8, 6, 6].as_slice()))
        );
        let result = cursor.get(None, None, MDB_NEXT);
        assert_eq!(
            result,
            Ok((Some([1u8, 2, 3].as_slice()), [4u8, 5, 6].as_slice()))
        );
        let result = cursor.get(None, None, MDB_NEXT);
        assert_eq!(result, Err(lmdb::Error::NotFound));
    }

    #[test]
    fn nulled_cursor_can_start_at_specified_key() {
        let env = LmdbEnv::new_null_with()
            .database("foo", LmdbDatabase::new_null(42))
            .entry(&[1, 1, 1], &[6, 6, 6])
            .entry(&[2, 2, 2], &[7, 7, 7])
            .entry(&[3, 3, 3], &[8, 8, 8])
            .build()
            .build();

        let txn = env.tx_begin_read();

        let cursor = txn
            .txn()
            .open_ro_cursor(LmdbDatabase::new_null(42))
            .unwrap();
        let result = cursor.get(Some([2u8, 2, 2].as_slice()), None, MDB_SET_RANGE);
        assert_eq!(
            result,
            Ok((Some([2u8, 2, 2].as_slice()), [7u8, 7, 7].as_slice()))
        );

        let result = cursor.get(Some([2u8, 1, 0].as_slice()), None, MDB_SET_RANGE);
        assert_eq!(
            result,
            Ok((Some([2u8, 2, 2].as_slice()), [7u8, 7, 7].as_slice()))
        );
    }

    #[test]
    fn nulled_cursor_can_be_iterated_forwards() {
        let env = LmdbEnv::new_null_with()
            .database("foo", LmdbDatabase::new_null(42))
            .entry(&[1, 2, 3], &[4, 5, 6])
            .entry(&[2, 2, 2], &[6, 6, 6])
            .build()
            .build();

        let txn = env.tx_begin_read();

        let cursor = txn
            .txn()
            .open_ro_cursor(LmdbDatabase::new_null(42))
            .unwrap();
        let result = cursor.get(None, None, MDB_FIRST);
        assert_eq!(
            result,
            Ok((Some([1u8, 2, 3].as_slice()), [4u8, 5, 6].as_slice()))
        );
        let result = cursor.get(None, None, MDB_NEXT);
        assert_eq!(
            result,
            Ok((Some([2u8, 2, 2].as_slice()), [6u8, 6, 6].as_slice()))
        );
        let result = cursor.get(None, None, MDB_NEXT);
        assert_eq!(result, Err(lmdb::Error::NotFound));
    }
}
