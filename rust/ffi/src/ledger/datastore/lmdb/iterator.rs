use rsnano_core::utils::{Deserialize, FixedSizeSerialize};
use rsnano_store_lmdb::{BinaryDbIterator, LmdbIteratorImpl};
use std::ffi::c_void;

#[repr(C)]
#[derive(Clone)]
pub struct MdbVal {
    pub mv_size: usize,       // size of the data item
    pub mv_data: *mut c_void, // address of the data item
}

impl MdbVal {
    pub fn new() -> Self {
        Self {
            mv_size: 0,
            mv_data: std::ptr::null_mut(),
        }
    }
}

pub struct LmdbIteratorHandle(LmdbIteratorImpl<'static>);

impl LmdbIteratorHandle {
    pub fn new2<K, V>(it: BinaryDbIterator<K, V>) -> *mut Self
    where
        K: FixedSizeSerialize + Deserialize<Target = K> + 'static,
        V: Deserialize<Target = V> + 'static,
    {
        Box::into_raw(Box::new(LmdbIteratorHandle(take_iterator_impl(it))))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_destroy(handle: *mut LmdbIteratorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_current(
    handle: &mut LmdbIteratorHandle,
    key: *mut MdbVal,
    value: *mut MdbVal,
) {
    match handle.0.current() {
        Some((k, v)) => {
            (*key).mv_size = k.len();
            (*key).mv_data = k.as_ptr() as *mut c_void;
            (*value).mv_size = v.len();
            (*value).mv_data = v.as_ptr() as *mut c_void;
        }
        None => {
            *key = MdbVal::new();
            *value = MdbVal::new();
        }
    }
}

#[no_mangle]
pub extern "C" fn rsn_lmdb_iterator_next(handle: &mut LmdbIteratorHandle) {
    handle.0.next();
}

pub(crate) fn take_iterator_impl<K, V>(mut it: BinaryDbIterator<K, V>) -> LmdbIteratorImpl<'static>
where
    K: FixedSizeSerialize + Deserialize<Target = K> + 'static,
    V: Deserialize<Target = V> + 'static,
{
    unsafe { std::mem::transmute::<LmdbIteratorImpl, LmdbIteratorImpl<'static>>(it.take_impl()) }
}
