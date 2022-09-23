use std::sync::{Mutex, MutexGuard};

use rand::{thread_rng, Rng};

use crate::RawKey;

/// The fan spreads a key out over the heap to decrease the likelihood of it being recovered by memory inspection
pub struct Fan {
    values: Mutex<Vec<Box<RawKey>>>,
}

impl Fan {
    pub fn new(key: RawKey, count: usize) -> Self {
        let mut first = Box::new(key);
        let mut values = Vec::with_capacity(count);
        let mut rng = thread_rng();
        for _ in 1..count {
            let entry = Box::new(RawKey::from_bytes(rng.gen()));
            *first.as_mut() ^= entry.as_ref().clone();
            values.push(entry);
        }
        values.push(first);

        Self {
            values: Mutex::new(values),
        }
    }

    pub fn value(&self) -> RawKey {
        let guard = self.values.lock().unwrap();
        get_fan_value(&guard)
    }

    pub fn value_set(&self, new_value: RawKey) {
        let mut guard = self.values.lock().unwrap();
        let old_value = get_fan_value(&guard);
        *guard[0] ^= old_value;
        *guard[0] ^= new_value;
    }
}

fn get_fan_value(guard: &MutexGuard<Vec<Box<RawKey>>>) -> RawKey {
    let mut key = RawKey::new();
    for i in guard.iter() {
        key ^= i.as_ref().clone();
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconstitute_fan() {
        let value0 = RawKey::from_bytes([0; 32]);
        let fan = Fan::new(value0, 1024);
        for i in fan.values.lock().unwrap().iter() {
            assert_ne!(i.as_ref(), &value0);
        }
        let value1 = fan.value();
        assert_eq!(value0, value1);
    }

    #[test]
    fn change_fan() {
        let value0 = RawKey::from_bytes([0; 32]);
        let value1 = RawKey::from_bytes([1; 32]);
        let fan = Fan::new(value0, 1024);
        assert_eq!(fan.values.lock().unwrap().len(), 1024);
        assert_eq!(fan.value(), value0);
        fan.value_set(value1);
        assert_eq!(fan.value(), value1);
    }
}
