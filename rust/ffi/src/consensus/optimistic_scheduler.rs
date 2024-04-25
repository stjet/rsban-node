use crate::{
    cementation::ConfirmingSetHandle, core::AccountInfoHandle, ledger::datastore::LedgerHandle,
    utils::ContainerInfoComponentHandle, ConfirmationHeightInfoDto, NetworkConstantsDto,
    OptimisticSchedulerConfigDto, StatHandle,
};
use rsnano_core::Account;
use rsnano_node::consensus::{OptimisticScheduler, OptimisticSchedulerExt};
use std::{
    ffi::{c_char, CStr},
    sync::Arc,
};

use super::ActiveTransactionsHandle;

pub struct OptimisticSchedulerHandle(Arc<OptimisticScheduler>);

#[no_mangle]
pub extern "C" fn rsn_optimistic_scheduler_create(
    config: &OptimisticSchedulerConfigDto,
    stats: &StatHandle,
    active: &ActiveTransactionsHandle,
    network_constants: &NetworkConstantsDto,
    ledger: &LedgerHandle,
    confirming_set: &ConfirmingSetHandle,
) -> *mut OptimisticSchedulerHandle {
    Box::into_raw(Box::new(OptimisticSchedulerHandle(Arc::new(
        OptimisticScheduler::new(
            config.into(),
            Arc::clone(stats),
            Arc::clone(active),
            network_constants.try_into().unwrap(),
            Arc::clone(ledger),
            Arc::clone(confirming_set),
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_optimistic_scheduler_destroy(handle: *mut OptimisticSchedulerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_optimistic_scheduler_start(handle: &OptimisticSchedulerHandle) {
    handle.0.start()
}

#[no_mangle]
pub extern "C" fn rsn_optimistic_scheduler_stop(handle: &OptimisticSchedulerHandle) {
    handle.0.stop()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_optimistic_scheduler_activate(
    handle: &OptimisticSchedulerHandle,
    account: *const u8,
    account_info: &AccountInfoHandle,
    conf_info: &ConfirmationHeightInfoDto,
) -> bool {
    handle
        .0
        .activate(Account::from_ptr(account), account_info, &conf_info.into())
}

#[no_mangle]
pub extern "C" fn rsn_optimistic_scheduler_notify(handle: &OptimisticSchedulerHandle) {
    handle.0.notify()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_optimistic_scheduler_collect_container_info(
    handle: &OptimisticSchedulerHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = handle
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
