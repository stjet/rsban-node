use crate::utils::{Deserialize, Serialize, StreamAdapter};

pub trait DbIteratorImpl: PartialEq {
    fn current(&self) -> Option<(&[u8], &[u8])>;
    fn next(&mut self);
}

pub struct DbIterator<K, V, I>
where
    K: Serialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
    I: DbIteratorImpl + PartialEq,
{
    iterator_impl: I,
    current: Option<(K, V)>,
}

impl<K, V, I> PartialEq for DbIterator<K, V, I>
where
    K: Serialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
    I: DbIteratorImpl + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.iterator_impl == other.iterator_impl
    }
}

impl<K, V, I> DbIterator<K, V, I>
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
                if k.len() < K::serialized_size() {
                    None
                } else {
                    let key = K::deserialize(&mut StreamAdapter::new(k)).unwrap();
                    let value = V::deserialize(&mut StreamAdapter::new(v)).unwrap();
                    Some((key, value))
                }
            }
            None => None,
        };
    }

    pub fn take_impl(self) -> I {
        self.iterator_impl
    }
}
