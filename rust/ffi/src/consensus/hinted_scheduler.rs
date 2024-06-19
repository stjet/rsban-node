use super::{vote_cache::VoteCacheHandle, ActiveTransactionsHandle};
use crate::{
    cementation::ConfirmingSetHandle, ledger::datastore::LedgerHandle,
    representatives::OnlineRepsHandle, HintedSchedulerConfigDto, StatHandle,
};
use rsnano_node::consensus::{HintedScheduler, HintedSchedulerExt};
use std::sync::Arc;

pub struct HintedSchedulerHandle(pub Arc<HintedScheduler>);

#[no_mangle]
pub unsafe extern "C" fn rsn_hinted_scheduler_destroy(handle: *mut HintedSchedulerHandle) {
    drop(Box::from_raw(handle));
}
