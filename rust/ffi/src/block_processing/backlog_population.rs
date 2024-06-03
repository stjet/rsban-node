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

pub struct BacklogPopulationHandle(pub Arc<BacklogPopulation>);

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_create(
    config_dto: *const BacklogPopulationConfigDto,
    ledger_handle: *mut LedgerHandle,
    stats_handle: *mut StatHandle,
) -> *mut BacklogPopulationHandle {
    Box::into_raw(Box::new(BacklogPopulationHandle(Arc::new(
        BacklogPopulation::new(
            (&*config_dto).into(),
            Arc::clone(&(*ledger_handle).0),
            Arc::clone(&(*stats_handle).0),
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_backlog_population_destroy(handle: *mut BacklogPopulationHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_backlog_population_trigger(handle: &BacklogPopulationHandle) {
    handle.0.trigger();
}

pub type BacklogPopulationActivateCallback =
    unsafe extern "C" fn(*mut c_void, *mut TransactionHandle, *const u8);

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
        .set_activate_callback(Box::new(move |txn, account| {
            let txn_handle = into_read_txn_handle(txn);

            (callback)(
                context_wrapper.get_context(),
                txn_handle,
                account.as_bytes().as_ptr(),
            );

            drop(Box::from_raw(txn_handle));
        }));
}
