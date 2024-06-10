use crate::{
    cementation::ConfirmingSetHandle, ledger::datastore::LedgerHandle, NetworkConstantsDto,
    OptimisticSchedulerConfigDto, StatHandle,
};
use rsnano_node::consensus::{OptimisticScheduler, OptimisticSchedulerExt};
use std::sync::Arc;

use super::ActiveTransactionsHandle;

pub struct OptimisticSchedulerHandle(pub Arc<OptimisticScheduler>);

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
pub extern "C" fn rsn_optimistic_scheduler_notify(handle: &OptimisticSchedulerHandle) {
    handle.0.notify()
}
