use crate::{nullable_lmdb::RoCursor, LmdbDatabase, Transaction};
use lmdb_sys::{MDB_FIRST, MDB_LAST, MDB_NEXT, MDB_SET_RANGE};
use rsnano_core::utils::{BufferReader, Deserialize, FixedSizeSerialize};
use std::ffi::c_uint;

pub struct BinaryDbIterator<'txn, K, V>
where
    K: FixedSizeSerialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
{
    iterator_impl: Option<LmdbIteratorImpl<'txn>>,
    current: Option<(K, V)>,
}

impl<'txn, K, V> PartialEq for BinaryDbIterator<'txn, K, V>
where
    K: FixedSizeSerialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
{
    fn eq(&self, other: &Self) -> bool {
        self.iterator_impl == other.iterator_impl
    }
}

impl<'txn, K, V> BinaryDbIterator<'txn, K, V>
where
    K: FixedSizeSerialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
{
    pub fn new(iterator_impl: LmdbIteratorImpl<'txn>) -> Self {
        let mut result = Self {
            iterator_impl: Some(iterator_impl),
            current: None,
        };
        result.load_current();
        result
    }

    fn load_current(&mut self) {
        self.current = match self.iterator_impl.as_ref().unwrap().current() {
            Some((k, v)) => {
                if k.len() < K::serialized_size() {
                    None
                } else {
                    let key = K::deserialize(&mut BufferReader::new(k)).unwrap();
                    let value = V::deserialize(&mut BufferReader::new(v)).unwrap();
                    Some((key, value))
                }
            }
            None => None,
        };
    }

    pub fn take_impl(&mut self) -> LmdbIteratorImpl<'txn> {
        self.iterator_impl.take().unwrap()
    }

    pub fn is_end(&self) -> bool {
        self.iterator_impl.as_ref().unwrap().current().is_none()
    }

    pub fn current(&self) -> Option<(&K, &V)> {
        self.current.as_ref().map(|(k, v)| (k, v))
    }

    pub fn next(&mut self) {
        self.iterator_impl.as_mut().unwrap().next();
        self.load_current();
    }

    pub fn eq(&self, other: &BinaryDbIterator<K, V>) -> bool {
        self.iterator_impl.eq(&other.iterator_impl)
    }
}

pub struct LmdbIteratorImpl<'txn> {
    current: Option<(&'txn [u8], &'txn [u8])>,
    cursor: Option<RoCursor<'txn>>,
}

impl<'txn> LmdbIteratorImpl<'txn> {
    pub fn new_iterator<K, V>(
        txn: &'txn dyn Transaction,
        dbi: LmdbDatabase,
        key_val: Option<&[u8]>,
        direction_asc: bool,
    ) -> BinaryDbIterator<'txn, K, V>
    where
        K: FixedSizeSerialize + Deserialize<Target = K> + 'static,
        V: Deserialize<Target = V> + 'static,
    {
        let iterator_impl = Self::new(txn, dbi, key_val, direction_asc);
        BinaryDbIterator::new(iterator_impl)
    }

    pub fn null_iterator<K, V>() -> BinaryDbIterator<'txn, K, V>
    where
        K: FixedSizeSerialize + Deserialize<Target = K> + 'static,
        V: Deserialize<Target = V> + 'static,
    {
        BinaryDbIterator::new(Self::null())
    }

    pub fn new(
        txn: &'txn dyn Transaction,
        dbi: LmdbDatabase,
        key_val: Option<&[u8]>,
        direction_asc: bool,
    ) -> LmdbIteratorImpl<'txn> {
        let operation = if key_val.is_some() {
            MDB_SET_RANGE
        } else if direction_asc {
            MDB_FIRST
        } else {
            MDB_LAST
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

    pub fn null() -> LmdbIteratorImpl<'static> {
        LmdbIteratorImpl::<'static> {
            current: None,
            cursor: None,
        }
    }

    pub fn current(&self) -> Option<(&[u8], &[u8])> {
        self.current
    }

    pub fn next(&mut self) {
        self.load_current(None, MDB_NEXT);
    }
}

impl<'txn> PartialEq for LmdbIteratorImpl<'txn> {
    fn eq(&self, other: &Self) -> bool {
        self.current.map(|(k, _)| k) == other.current.map(|(k, _)| k)
    }
}
