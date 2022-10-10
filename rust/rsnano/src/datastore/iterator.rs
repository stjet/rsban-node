use crate::utils::{Deserialize, Serialize, StreamAdapter};

use super::lmdb::LmdbRawIterator;

pub trait DbIterator<K, V> {
    fn take_lmdb_raw_iterator(&mut self) -> Option<LmdbRawIterator>;
    fn current(&self) -> Option<(&K, &V)>;
    fn next(&mut self);
    fn is_end(&self) -> bool;
}

pub struct NullIterator {}

impl NullIterator {
    pub fn new() -> Self {
        Self {}
    }
}

impl<K, V> DbIterator<K, V> for NullIterator {
    fn take_lmdb_raw_iterator(&mut self) -> Option<LmdbRawIterator> {
        None
    }

    fn is_end(&self) -> bool {
        true
    }

    fn current(&self) -> Option<(&K, &V)> {
        None
    }

    fn next(&mut self) {}
}

pub trait DbIteratorImpl {
    fn current(&self) -> Option<(&[u8], &[u8])>;
    fn next(&mut self);
}

pub struct DbIterator2<K, V, I>
where
    K: Serialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
    I: DbIteratorImpl + PartialEq,
{
    iterator_impl: I,
    current: Option<(K, V)>,
}

impl<K, V, I> PartialEq for DbIterator2<K, V, I>
where
    K: Serialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
    I: DbIteratorImpl + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.iterator_impl == other.iterator_impl
    }
}

impl<K, V, I> DbIterator2<K, V, I>
where
    K: Serialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
    I: DbIteratorImpl + PartialEq,
{
    pub fn new(iterator_impl: I) -> Self {
        let mut result = Self {
            iterator_impl,
            current: None,
        };
        result.load_current();
        result
    }

    pub fn is_end(&self) -> bool {
        self.iterator_impl.current().is_none()
    }

    pub fn current(&self) -> Option<(&K, &V)> {
        self.current.as_ref().map(|(k, v)| (k, v))
    }

    pub fn next(&mut self) {
        self.iterator_impl.next();
        self.load_current();
    }

    fn load_current(&mut self) {
        self.current = match self.iterator_impl.current() {
            Some((k, v)) => {
                let key = K::deserialize(&mut StreamAdapter::new(k)).unwrap();
                let value = V::deserialize(&mut StreamAdapter::new(v)).unwrap();
                Some((key, value))
            }
            None => None,
        };
    }

    pub fn take_impl(self) -> I {
        self.iterator_impl
    }
}
