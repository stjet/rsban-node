use crate::{nullable_lmdb::RoCursor, LmdbDatabase, Transaction};
use lmdb_sys::{MDB_FIRST, MDB_LAST, MDB_NEXT, MDB_SET_RANGE};
use rsnano_core::utils::{BufferReader, Deserialize, FixedSizeSerialize};
use std::{any::Any, ffi::c_uint};

pub trait DbIterator<K, V> {
    fn is_end(&self) -> bool;
    fn current(&self) -> Option<(&K, &V)>;
    fn next(&mut self);
    fn eq(&self, other: &dyn DbIterator<K, V>) -> bool;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait DbIteratorImpl: PartialEq {
    fn current(&self) -> Option<(&[u8], &[u8])>;
    fn next(&mut self);
}

pub struct BinaryDbIterator<K, V, I>
where
    K: FixedSizeSerialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
    I: DbIteratorImpl + PartialEq,
{
    iterator_impl: Option<I>,
    current: Option<(K, V)>,
}

impl<K, V, I> PartialEq for BinaryDbIterator<K, V, I>
where
    K: FixedSizeSerialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
    I: DbIteratorImpl + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.iterator_impl == other.iterator_impl
    }
}

impl<K, V, I> BinaryDbIterator<K, V, I>
where
    K: FixedSizeSerialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
    I: DbIteratorImpl + PartialEq,
{
    pub fn new(iterator_impl: I) -> Self {
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

    pub fn take_impl(&mut self) -> I {
        self.iterator_impl.take().unwrap()
    }
}

impl<K, V, I> DbIterator<K, V> for BinaryDbIterator<K, V, I>
where
    K: FixedSizeSerialize + Deserialize<Target = K> + 'static,
    V: Deserialize<Target = V> + 'static,
    I: DbIteratorImpl + PartialEq + 'static,
{
    fn is_end(&self) -> bool {
        self.iterator_impl.as_ref().unwrap().current().is_none()
    }

    fn current(&self) -> Option<(&K, &V)> {
        self.current.as_ref().map(|(k, v)| (k, v))
    }

    fn next(&mut self) {
        self.iterator_impl.as_mut().unwrap().next();
        self.load_current();
    }

    fn eq(&self, other: &dyn DbIterator<K, V>) -> bool {
        let other = other
            .as_any()
            .downcast_ref::<BinaryDbIterator<K, V, I>>()
            .unwrap();
        self.iterator_impl.eq(&other.iterator_impl)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct LmdbIteratorImpl {
    current: Option<(&'static [u8], &'static [u8])>,
    cursor: Option<RoCursor>,
}

impl LmdbIteratorImpl {
    pub fn new_iterator<K, V>(
        txn: &dyn Transaction,
        dbi: LmdbDatabase,
        key_val: Option<&[u8]>,
        direction_asc: bool,
    ) -> Box<dyn DbIterator<K, V>>
    where
        K: FixedSizeSerialize + Deserialize<Target = K> + 'static,
        V: Deserialize<Target = V> + 'static,
    {
        let iterator_impl = Self::new(txn, dbi, key_val, direction_asc);
        Box::new(BinaryDbIterator::new(iterator_impl))
    }

    pub fn null_iterator<K, V>() -> Box<dyn DbIterator<K, V>>
    where
        K: FixedSizeSerialize + Deserialize<Target = K> + 'static,
        V: Deserialize<Target = V> + 'static,
    {
        Box::new(BinaryDbIterator::new(Self::null()))
    }

    pub fn new(
        txn: &dyn Transaction,
        dbi: LmdbDatabase,
        key_val: Option<&[u8]>,
        direction_asc: bool,
    ) -> Self {
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

impl PartialEq for LmdbIteratorImpl {
    fn eq(&self, other: &Self) -> bool {
        self.current.map(|(k, _)| k) == other.current.map(|(k, _)| k)
    }
}
