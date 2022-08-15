use crate::datastore::lmdb::LmdbIterator;
use std::ffi::c_void;

pub struct LmdbIteratorHandle(LmdbIterator);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_create(
    txn: *mut c_void,
    dbi: u32,
) -> *mut LmdbIteratorHandle {
    Box::into_raw(Box::new(LmdbIteratorHandle(LmdbIterator::new(txn, dbi))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_destroy(handle: *mut LmdbIteratorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_cursor(handle: *mut LmdbIteratorHandle) -> *mut c_void {
    (*handle).0.cursor()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_set_cursor(
    handle: *mut LmdbIteratorHandle,
    cursor: *mut c_void,
) {
    (*handle).0.set_cursor(cursor);
}
