use super::ActiveTransactionsHandle;
use crate::{core::BlockHandle, StatHandle};
use num_traits::FromPrimitive;
use rsnano_core::Amount;
use rsnano_node::consensus::{ManualScheduler, ManualSchedulerExt};
use std::sync::Arc;

pub struct ManualSchedulerHandle(pub Arc<ManualScheduler>);

#[no_mangle]
pub extern "C" fn rsn_manual_scheduler_create(
    stats: &StatHandle,
    active: &ActiveTransactionsHandle,
) -> *mut ManualSchedulerHandle {
    Box::into_raw(Box::new(ManualSchedulerHandle(Arc::new(
        ManualScheduler::new(Arc::clone(stats), Arc::clone(active)),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_manual_scheduler_destroy(handle: *mut ManualSchedulerHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_manual_scheduler_start(handle: &ManualSchedulerHandle) {
    handle.0.start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_manual_scheduler_stop(handle: &ManualSchedulerHandle) {
    handle.0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_manual_scheduler_push(
    handle: &ManualSchedulerHandle,
    block: &BlockHandle,
    previous_balance: *const u8,
) {
    let previous_balance = if previous_balance.is_null() {
        None
    } else {
        Some(Amount::from_ptr(previous_balance))
    };
    handle.0.push(Arc::clone(block), previous_balance);
}
