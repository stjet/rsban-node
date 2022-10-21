use std::ffi::c_void;

use crate::{
    ffi::VoidPointerCallback,
    ledger::datastore::{lmdb::LmdbIteratorImpl, DbIterator, DbIteratorImpl, ReadTransaction},
    utils::{Deserialize, Serialize},
};

use super::{TransactionHandle, TransactionType};
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

enum IteratorType {
    Lmdb(LmdbIteratorImpl),
}

pub struct LmdbIteratorHandle(IteratorType);

impl LmdbIteratorHandle {
    pub fn new(it: LmdbIteratorImpl) -> *mut Self {
        Box::into_raw(Box::new(LmdbIteratorHandle(IteratorType::Lmdb(it))))
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
        IteratorType::Lmdb(h) => match h.current() {
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
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_iterator_next(handle: *mut LmdbIteratorHandle) {
    match &mut (*handle).0 {
        IteratorType::Lmdb(h) => h.next(),
    }
}

pub fn to_lmdb_iterator_handle2<K, V>(
    iterator: DbIterator<K, V, LmdbIteratorImpl>,
) -> *mut LmdbIteratorHandle
where
    K: Serialize + Deserialize<Target = K>,
    V: Deserialize<Target = V>,
{
    LmdbIteratorHandle::new(iterator.take_impl())
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
    pub fn execute<K, V>(
        &self,
        txn: &dyn ReadTransaction,
        begin: DbIterator<K, V, LmdbIteratorImpl>,
        end: DbIterator<K, V, LmdbIteratorImpl>,
    ) where
        K: Serialize + Deserialize<Target = K>,
        V: Deserialize<Target = V>,
    {
        let lmdb_txn = unsafe {
            std::mem::transmute::<&dyn ReadTransaction, &'static dyn ReadTransaction>(txn)
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
