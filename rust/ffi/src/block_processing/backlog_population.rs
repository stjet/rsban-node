use std::{ffi::c_void, sync::Arc};

use crate::{
    core::AccountInfoHandle,
    ledger::datastore::{into_read_txn_handle, LedgerHandle, TransactionHandle},
    utils::ContextWrapper,
    ConfirmationHeightInfoDto, StatHandle, VoidPointerCallback,
};
use rsnano_node::block_processing::{BacklogPopulation, BacklogPopulationConfig};

#[repr(C)]
pub struct BacklogPopulationConfigDto {
    pub enabled: bool,
    pub batch_size: u32,
    pub frequency: u32,
}

impl From<&BacklogPopulationConfigDto> for BacklogPopulationConfig {
    fn from(value: &BacklogPopulationConfigDto) -> Self {
        Self {
            enabled: value.enabled,
            batch_size: value.batch_size,
            frequency: value.frequency,
        }
    }
}

pub struct BacklogPopulationHandle(BacklogPopulation);

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_create(
    config_dto: *const BacklogPopulationConfigDto,
    ledger_handle: *mut LedgerHandle,
    stats_handle: *mut StatHandle,
) -> *mut BacklogPopulationHandle {
    Box::into_raw(Box::new(BacklogPopulationHandle(BacklogPopulation::new(
        (&*config_dto).into(),
        Arc::clone(&(*ledger_handle).0),
        Arc::clone(&(*stats_handle).0),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_destroy(handle: *mut BacklogPopulationHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_start(handle: *mut BacklogPopulationHandle) {
    (*handle).0.start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_stop(handle: *mut BacklogPopulationHandle) {
    (*handle).0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_trigger(handle: *mut BacklogPopulationHandle) {
    (*handle).0.trigger();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_notify(handle: *mut BacklogPopulationHandle) {
    (*handle).0.notify();
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
            let txn_handle = into_read_txn_handle(txn);

            let account_info_handle =
                Box::into_raw(Box::new(AccountInfoHandle(account_info.clone())));
            let conf_height_dto = conf_height.into();

            (callback)(
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
