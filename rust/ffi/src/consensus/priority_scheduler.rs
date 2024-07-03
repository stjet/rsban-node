use crate::ledger::datastore::TransactionHandle;
use rsnano_core::Account;
use rsnano_node::consensus::PriorityScheduler;
use std::{ops::Deref, sync::Arc};

pub struct ElectionSchedulerHandle(pub Arc<PriorityScheduler>);

impl Deref for ElectionSchedulerHandle {
    type Target = Arc<PriorityScheduler>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_scheduler_destroy(handle: *mut ElectionSchedulerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_scheduler_activate(
    handle: &ElectionSchedulerHandle,
    account: *const u8,
    tx: &TransactionHandle,
) -> bool {
    handle.0.activate(tx.as_txn(), &Account::from_ptr(account))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_scheduler_len(handle: &ElectionSchedulerHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_scheduler_empty(handle: &ElectionSchedulerHandle) -> bool {
    handle.0.is_empty()
}
