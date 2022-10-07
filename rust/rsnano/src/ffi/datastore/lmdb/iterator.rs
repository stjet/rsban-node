use std::{ffi::c_void, ptr};

use crate::{
    datastore::{
        lmdb::{LmdbRawIterator, LmdbReadTransaction, MdbVal},
        DbIterator, DbIterator2,
    },
    ffi::VoidPointerCallback,
    utils::{Deserialize, Serialize},
};

use super::{TransactionHandle, TransactionType};

enum IteratorType {
    Lmdb(LmdbRawIterator),
    Rkv(crate::datastore::lmdb_rkv::LmdbIteratorImpl),
}

pub struct LmdbIteratorHandle(IteratorType);

impl LmdbIteratorHandle {
    //todo delete
    pub fn new(it: LmdbRawIterator) -> *mut Self {
        Box::into_raw(Box::new(LmdbIteratorHandle(IteratorType::Lmdb(it))))
    }

    pub fn new2(it: crate::datastore::lmdb_rkv::LmdbIteratorImpl) -> *mut Self {
        Box::into_raw(Box::new(LmdbIteratorHandle(IteratorType::Rkv(it))))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_destroy(handle: *mut LmdbIteratorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_current(
    handle: *mut LmdbIteratorHandle,
    key: *mut MdbVal,
    value: *mut MdbVal,
) {
    match &(*handle).0 {
        IteratorType::Lmdb(h) => {
            *key = h.key.clone();
            *value = h.value.clone();
        }
        IteratorType::Rkv(_) => todo!(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_next(handle: *mut LmdbIteratorHandle) {
    match &mut (*handle).0 {
        IteratorType::Lmdb(h) => h.next(),
        IteratorType::Rkv(_) => todo!(),
    }
}

//todo delete
pub fn to_lmdb_iterator_handle<K, V>(
    iterator: &mut dyn DbIterator<K, V>,
) -> *mut LmdbIteratorHandle {
    match iterator.take_lmdb_raw_iterator() {
        Some(it) => LmdbIteratorHandle::new(it),
        None => ptr::null_mut(),
    }
}

pub fn to_lmdb_iterator_handle2<K, V>(
    iterator: DbIterator2<K, V, crate::datastore::lmdb::LmdbIteratorImpl>,
) -> *mut LmdbIteratorHandle
where
    K: Serialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
{
    LmdbIteratorHandle::new(iterator.take_impl().take_raw_iterator())
}

pub type ForEachParCallback = extern "C" fn(
    *mut c_void,
    *mut TransactionHandle,
    *mut LmdbIteratorHandle,
    *mut LmdbIteratorHandle,
);

pub struct ForEachParWrapper {
    pub action: ForEachParCallback,
    pub context: *mut c_void,
    pub delete_context: VoidPointerCallback,
}

impl ForEachParWrapper {
    //todo delete
    pub fn execute<K, V>(
        &self,
        txn: &LmdbReadTransaction,
        begin: &mut dyn DbIterator<K, V>,
        end: &mut dyn DbIterator<K, V>,
    ) {
        let lmdb_txn = unsafe {
            std::mem::transmute::<&LmdbReadTransaction, &'static LmdbReadTransaction>(txn)
        };
        let txn_handle = TransactionHandle::new(TransactionType::ReadRef(lmdb_txn));
        let begin_handle = to_lmdb_iterator_handle(begin);
        let end_handle = to_lmdb_iterator_handle(end);
        (self.action)(self.context, txn_handle, begin_handle, end_handle);
    }

    pub fn execute2<K, V>(
        &self,
        txn: &LmdbReadTransaction,
        begin: DbIterator2<K, V, crate::datastore::lmdb::LmdbIteratorImpl>,
        end: DbIterator2<K, V, crate::datastore::lmdb::LmdbIteratorImpl>,
    ) where
        K: Serialize + Deserialize<Target = K>,
        V: Deserialize<Target = V>,
    {
        let lmdb_txn = unsafe {
            std::mem::transmute::<&LmdbReadTransaction, &'static LmdbReadTransaction>(txn)
        };
        let txn_handle = TransactionHandle::new(TransactionType::ReadRef(lmdb_txn));
        let begin_handle = to_lmdb_iterator_handle2(begin);
        let end_handle = to_lmdb_iterator_handle2(end);
        (self.action)(self.context, txn_handle, begin_handle, end_handle);
    }
}

unsafe impl Send for ForEachParWrapper {}
unsafe impl Sync for ForEachParWrapper {}

impl Drop for ForEachParWrapper {
    fn drop(&mut self) {
        unsafe { (self.delete_context)(self.context) }
    }
}
