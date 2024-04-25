use super::{vote_cache::VoteCacheHandle, ActiveTransactionsHandle};
use crate::{
    cementation::ConfirmingSetHandle, ledger::datastore::LedgerHandle,
    representatives::OnlineRepsHandle, utils::ContainerInfoComponentHandle,
    HintedSchedulerConfigDto, StatHandle,
};
use rsnano_node::consensus::{HintedScheduler, HintedSchedulerExt};
use std::{
    ffi::{c_char, CStr},
    sync::Arc,
};

pub struct HintedSchedulerHandle(Arc<HintedScheduler>);

#[no_mangle]
pub extern "C" fn rsn_hinted_scheduler_create(
    config: &HintedSchedulerConfigDto,
    active: &ActiveTransactionsHandle,
    ledger: &LedgerHandle,
    stats: &StatHandle,
    vote_cache: &VoteCacheHandle,
    confirming_set: &ConfirmingSetHandle,
    online_reps: &OnlineRepsHandle,
) -> *mut HintedSchedulerHandle {
    Box::into_raw(Box::new(HintedSchedulerHandle(Arc::new(
        HintedScheduler::new(
            config.into(),
            Arc::clone(active),
            Arc::clone(ledger),
            Arc::clone(stats),
            Arc::clone(vote_cache),
            Arc::clone(confirming_set),
            Arc::clone(online_reps),
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hinted_scheduler_destroy(handle: *mut HintedSchedulerHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub extern "C" fn rsn_hinted_scheduler_start(handle: &HintedSchedulerHandle) {
    handle.0.start();
}

#[no_mangle]
pub extern "C" fn rsn_hinted_scheduler_stop(handle: &HintedSchedulerHandle) {
    handle.0.stop();
}

#[no_mangle]
pub extern "C" fn rsn_hinted_scheduler_notify(handle: &HintedSchedulerHandle) {
    handle.0.notify();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hinted_scheduler_collect_container_info(
    handle: &HintedSchedulerHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = handle
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
