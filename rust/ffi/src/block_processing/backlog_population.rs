use std::{ffi::c_void, sync::Arc};

use crate::{
    core::AccountInfoHandle,
    ledger::datastore::{LmdbStoreHandle, TransactionHandle, TransactionType},
    utils::ContextWrapper,
    ConfirmationHeightInfoDto, StatHandle, VoidPointerCallback,
};
use rsnano_core::Account;
use rsnano_node::block_processing::BacklogPopulation;
use rsnano_store_lmdb::LmdbReadTransaction;

pub struct BacklogPopulationHandle(BacklogPopulation);

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_create(
    store_handle: *mut LmdbStoreHandle,
    stats_handle: *mut StatHandle,
) -> *mut BacklogPopulationHandle {
    Box::into_raw(Box::new(BacklogPopulationHandle(BacklogPopulation::new(
        Arc::clone(&(*store_handle).0),
        Arc::clone(&(*stats_handle).0),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_destroy(handle: *mut BacklogPopulationHandle) {
    drop(Box::from_raw(handle))
}

pub type BacklogPopulationActivateCallback = unsafe extern "C" fn(
    *mut c_void,
    *mut TransactionHandle,
    *const u8,
    *mut AccountInfoHandle,
    *const ConfirmationHeightInfoDto,
);

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_set_activate_callback(
    handle: *mut BacklogPopulationHandle,
    context: *mut c_void,
    callback: BacklogPopulationActivateCallback,
    delete_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(context, delete_context);
    (*handle)
        .0
        .set_activate_callback(Box::new(move |txn, account, account_info, conf_height| {
            let txn_handle = TransactionHandle::new(TransactionType::ReadRef(unsafe {
                std::mem::transmute::<&LmdbReadTransaction, &'static LmdbReadTransaction>(
                    txn.as_any().downcast_ref::<LmdbReadTransaction>().unwrap(),
                )
            }));

            let account_info_handle =
                Box::into_raw(Box::new(AccountInfoHandle(account_info.clone())));
            let conf_height_dto = conf_height.into();
            callback(
                context_wrapper.get_context(),
                txn_handle,
                account.as_bytes().as_ptr(),
                account_info_handle,
                &conf_height_dto,
            );

            drop(Box::from_raw(txn_handle));
            drop(Box::from_raw(account_info_handle));
        }));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_activate(
    handle: *mut BacklogPopulationHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
) {
    (*handle)
        .0
        .activate((*txn).as_txn(), &Account::from_ptr(account));
}
