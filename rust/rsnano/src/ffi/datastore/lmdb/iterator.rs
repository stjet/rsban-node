use std::ptr;

use crate::datastore::{
    lmdb::{LmdbRawIterator, MdbCursor, MdbTxn, MdbVal},
    DbIterator,
};

pub struct LmdbIteratorHandle(LmdbRawIterator);

impl LmdbIteratorHandle {
    pub fn new(it: LmdbRawIterator) -> *mut Self {
        Box::into_raw(Box::new(LmdbIteratorHandle(it)))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_create(
    txn: *mut MdbTxn,
    dbi: u32,
    val: *const MdbVal,
    direction_asc: bool,
    expected_value_size: usize,
) -> *mut LmdbIteratorHandle {
    LmdbIteratorHandle::new(LmdbRawIterator::new(
        txn,
        dbi,
        &*val,
        direction_asc,
        expected_value_size,
    ))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_destroy(handle: *mut LmdbIteratorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_cursor(
    handle: *mut LmdbIteratorHandle,
) -> *mut MdbCursor {
    (*handle).0.cursor()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_current(
    handle: *mut LmdbIteratorHandle,
    key: *mut MdbVal,
    value: *mut MdbVal,
) {
    *key = (*handle).0.key.clone();
    *value = (*handle).0.value.clone();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_next(handle: *mut LmdbIteratorHandle) {
    (*handle).0.next();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_previous(handle: *mut LmdbIteratorHandle) {
    (*handle).0.previous();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_clear(handle: *mut LmdbIteratorHandle) {
    (*handle).0.clear();
}

pub fn to_lmdb_iterator_handle<K, V>(
    iterator: &mut dyn DbIterator<K, V>,
) -> *mut LmdbIteratorHandle {
    match iterator.take_lmdb_raw_iterator() {
        Some(it) => LmdbIteratorHandle::new(it),
        None => ptr::null_mut(),
    }
}
