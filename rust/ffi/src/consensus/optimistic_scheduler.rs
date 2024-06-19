use super::ActiveTransactionsHandle;
use crate::{
    cementation::ConfirmingSetHandle, ledger::datastore::LedgerHandle, NetworkConstantsDto,
    OptimisticSchedulerConfigDto, StatHandle,
};
use rsnano_node::consensus::{OptimisticScheduler, OptimisticSchedulerExt};
use std::sync::Arc;

pub struct OptimisticSchedulerHandle(pub Arc<OptimisticScheduler>);

#[no_mangle]
pub unsafe extern "C" fn rsn_optimistic_scheduler_destroy(handle: *mut OptimisticSchedulerHandle) {
    drop(Box::from_raw(handle))
}
