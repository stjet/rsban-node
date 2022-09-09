use std::{
    ffi::c_void,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use crate::{
    datastore::{
        lmdb::{LmdbAccountStore, LmdbReadTransaction},
        AccountStore, DbIterator, ReadTransaction,
    },
    ffi::{AccountInfoHandle, VoidPointerCallback},
    Account, AccountInfo,
};

use super::{
    iterator::{to_lmdb_iterator_handle, LmdbIteratorHandle},
    lmdb_env::LmdbEnvHandle,
    TransactionHandle, TransactionType,
};

pub struct LmdbAccountStoreHandle(LmdbAccountStore);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_create(
    env_handle: *mut LmdbEnvHandle,
) -> *mut LmdbAccountStoreHandle {
    Box::into_raw(Box::new(LmdbAccountStoreHandle(LmdbAccountStore::new(
        Arc::clone(&*env_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_destroy(handle: *mut LmdbAccountStoreHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_accounts_handle(
    handle: *mut LmdbAccountStoreHandle,
) -> u32 {
    (*handle).0.accounts_handle
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_open_databases(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
    flags: u32,
) -> bool {
    (*handle).0.open_databases((*txn).as_txn(), flags).is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_put(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
    info: *const AccountInfoHandle,
) {
    let account = Account::from_ptr(account);
    let info = (*info).deref();
    (*handle).0.put((*txn).as_write_txn(), &account, info);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_get(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
    info: *mut AccountInfoHandle,
) -> bool {
    let account = Account::from_ptr(account);
    let info = (*info).deref_mut();
    match (*handle).0.get((*txn).as_txn(), &account) {
        Some(i) => {
            *info = i;
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_del(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
) {
    let account = Account::from_ptr(account);
    (*handle).0.del((*txn).as_write_txn(), &account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_begin_account(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
) -> *mut LmdbIteratorHandle {
    let account = Account::from_ptr(account);
    let mut iterator = (*handle).0.begin_account((*txn).as_txn(), &account);
    to_lmdb_iterator_handle(iterator.as_mut())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_begin(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
) -> *mut LmdbIteratorHandle {
    let mut iterator = (*handle).0.begin((*txn).as_txn());
    to_lmdb_iterator_handle(iterator.as_mut())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_rbegin(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
) -> *mut LmdbIteratorHandle {
    let mut iterator = (*handle).0.rbegin((*txn).as_txn());
    to_lmdb_iterator_handle(iterator.as_mut())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_count(
    handle: *mut LmdbAccountStoreHandle,
    txn: *mut TransactionHandle,
) -> usize {
    (*handle).0.count((*txn).as_txn())
}
pub type AccountStoreForEachParCallback = extern "C" fn(
    *mut c_void,
    *mut TransactionHandle,
    *mut LmdbIteratorHandle,
    *mut LmdbIteratorHandle,
);

struct ForEachParWrapper {
    action: AccountStoreForEachParCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
}

impl ForEachParWrapper {
    pub fn execute(
        &self,
        txn: &dyn ReadTransaction,
        begin: &mut dyn DbIterator<Account, AccountInfo>,
        end: &mut dyn DbIterator<Account, AccountInfo>,
    ) {
        let lmdb_txn = txn.as_any().downcast_ref::<LmdbReadTransaction>().unwrap();
        let lmdb_txn = unsafe {
            std::mem::transmute::<&LmdbReadTransaction, &'static LmdbReadTransaction>(lmdb_txn)
        };
        let txn_handle = TransactionHandle::new(TransactionType::ReadRef(lmdb_txn));
        let begin_handle = to_lmdb_iterator_handle(begin);
        let end_handle = to_lmdb_iterator_handle(end);
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

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_account_store_for_each_par(
    handle: *mut LmdbAccountStoreHandle,
    action: AccountStoreForEachParCallback,
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
