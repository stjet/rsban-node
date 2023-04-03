use std::sync::Arc;

use rsnano_core::{BlockHash, HashOrAccount, UncheckedKey};
use rsnano_store_lmdb::LmdbUncheckedStore;
use rsnano_store_traits::UncheckedStore;

use crate::core::UncheckedInfoHandle;

use super::{iterator::LmdbIteratorHandle, TransactionHandle};

pub struct LmdbUncheckedStoreHandle(Arc<LmdbUncheckedStore>);

impl LmdbUncheckedStoreHandle {
    pub fn new(store: Arc<LmdbUncheckedStore>) -> *mut Self {
        Box::into_raw(Box::new(LmdbUncheckedStoreHandle(store)))
    }
}

#[repr(C)]
pub struct UncheckedKeyDto {
    pub previous: [u8; 32],
    pub hash: [u8; 32],
}

impl From<&UncheckedKeyDto> for UncheckedKey {
    fn from(dto: &UncheckedKeyDto) -> Self {
        Self {
            previous: BlockHash::from_bytes(dto.previous),
            hash: BlockHash::from_bytes(dto.hash),
        }
    }
}

impl From<&UncheckedKey> for UncheckedKeyDto {
    fn from(key: &UncheckedKey) -> Self {
        Self {
            previous: *key.previous.as_bytes(),
            hash: *key.hash.as_bytes(),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_destroy(handle: *mut LmdbUncheckedStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_clear(
    handle: *mut LmdbUncheckedStoreHandle,
    txn: *mut TransactionHandle,
) {
    (*handle).0.clear((*txn).as_write_txn());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_put(
    handle: *mut LmdbUncheckedStoreHandle,
    txn: *mut TransactionHandle,
    dependency: *const u8,
    info: *mut UncheckedInfoHandle,
) {
    (*handle).0.put(
        (*txn).as_write_txn(),
        &HashOrAccount::from_ptr(dependency),
        &*info,
    );
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_exists(
    handle: *mut LmdbUncheckedStoreHandle,
    txn: *mut TransactionHandle,
    key: *const UncheckedKeyDto,
) -> bool {
    (*handle)
        .0
        .exists((*txn).as_txn(), &UncheckedKey::from(&*key))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_del(
    handle: *mut LmdbUncheckedStoreHandle,
    txn: *mut TransactionHandle,
    key: *const UncheckedKeyDto,
) {
    (*handle)
        .0
        .del((*txn).as_write_txn(), &UncheckedKey::from(&*key));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_begin(
    handle: *mut LmdbUncheckedStoreHandle,
    txn: *mut TransactionHandle,
) -> *mut LmdbIteratorHandle {
    let iterator = (*handle).0.begin((*txn).as_txn());
    LmdbIteratorHandle::new2(iterator)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_lower_bound(
    handle: *mut LmdbUncheckedStoreHandle,
    txn: *mut TransactionHandle,
    key: *const UncheckedKeyDto,
) -> *mut LmdbIteratorHandle {
    let key = UncheckedKey::from(&*key);
    let iterator = (*handle).0.lower_bound((*txn).as_txn(), &key);
    LmdbIteratorHandle::new2(iterator)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_unchecked_store_count(
    handle: *mut LmdbUncheckedStoreHandle,
    txn: *mut TransactionHandle,
) -> usize {
    (*handle).0.count((*txn).as_txn()) as usize
}
