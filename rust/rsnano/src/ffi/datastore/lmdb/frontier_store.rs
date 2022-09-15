use std::{ffi::c_void, sync::Arc};

use crate::{
    datastore::{lmdb::LmdbFrontierStore, FrontierStore},
    ffi::{copy_account_bytes, VoidPointerCallback},
    Account, BlockHash,
};

use super::{
    iterator::{
        to_lmdb_iterator_handle, ForEachParCallback, ForEachParWrapper, LmdbIteratorHandle,
    },
    lmdb_env::LmdbEnvHandle,
    TransactionHandle,
};

pub struct LmdbFrontierStoreHandle(LmdbFrontierStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbFrontierStoreHandle {
    Box::into_raw(Box::new(LmdbFrontierStoreHandle(LmdbFrontierStore::new(
        Arc::clone(&*env_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_destroy(handle: *mut LmdbFrontierStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_table_handle(
    handle: *mut LmdbFrontierStoreHandle,
) -> u32 {
    (*handle).0.table_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_set_table_handle(
    handle: *mut LmdbFrontierStoreHandle,
    table_handle: u32,
) {
    (*handle).0.table_handle = table_handle;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_put(
    handle: *mut LmdbFrontierStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
    account: *const u8,
) {
    (*handle).0.put(
        (*txn).as_write_txn(),
        &BlockHash::from_ptr(hash),
        &Account::from_ptr(account),
    );
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_get(
    handle: *mut LmdbFrontierStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
    account: *mut u8,
) {
    let result = (*handle).0.get((*txn).as_txn(), &BlockHash::from_ptr(hash));
    copy_account_bytes(result, account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_del(
    handle: *mut LmdbFrontierStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
) {
    (*handle)
        .0
        .del((*txn).as_write_txn(), &BlockHash::from_ptr(hash));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_begin(
    handle: *mut LmdbFrontierStoreHandle,
    txn: *mut TransactionHandle,
) -> *mut LmdbIteratorHandle {
    let mut iterator = (*handle).0.begin((*txn).as_txn());
    to_lmdb_iterator_handle(iterator.as_mut())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_begin_at_hash(
    handle: *mut LmdbFrontierStoreHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
) -> *mut LmdbIteratorHandle {
    let hash = BlockHash::from_ptr(hash);
    let mut iterator = (*handle).0.begin_at_hash((*txn).as_txn(), &hash);
    to_lmdb_iterator_handle(iterator.as_mut())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_frontier_store_for_each_par(
    handle: *mut LmdbFrontierStoreHandle,
    action: ForEachParCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let wrapper = ForEachParWrapper {
        action,
        context,
        delete_context,
    };
    (*handle)
        .0
        .for_each_par(&|txn, begin, end| wrapper.execute(txn, begin, end));
}
