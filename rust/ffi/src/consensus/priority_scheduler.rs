use super::ActiveTransactionsHandle;
use crate::{
    core::BlockHandle,
    ledger::datastore::{LedgerHandle, TransactionHandle},
    StatHandle,
};
use rsnano_core::Account;
use rsnano_node::consensus::{PriorityScheduler, PrioritySchedulerExt};
use std::{ops::Deref, sync::Arc};

pub struct ElectionSchedulerHandle(pub Arc<PriorityScheduler>);

impl Deref for ElectionSchedulerHandle {
    type Target = Arc<PriorityScheduler>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_election_scheduler_create(
    ledger: &LedgerHandle,
    stats: &StatHandle,
    active: &ActiveTransactionsHandle,
) -> *mut ElectionSchedulerHandle {
    Box::into_raw(Box::new(ElectionSchedulerHandle(Arc::new(
        PriorityScheduler::new(Arc::clone(ledger), Arc::clone(stats), Arc::clone(active)),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_scheduler_destroy(handle: *mut ElectionSchedulerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_election_scheduler_start(handle: &ElectionSchedulerHandle) {
    handle.0.start();
}

#[no_mangle]
pub extern "C" fn rsn_election_scheduler_stop(handle: &ElectionSchedulerHandle) {
    handle.0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_scheduler_activate(
    handle: &ElectionSchedulerHandle,
    account: *const u8,
    tx: &TransactionHandle,
) -> bool {
    handle.0.activate(&Account::from_ptr(account), tx.as_txn())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_scheduler_activate_successors(
    handle: &ElectionSchedulerHandle,
    tx: &mut TransactionHandle,
    block: &BlockHandle,
) {
    handle.0.activate_successors(tx.as_read_txn(), block);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_scheduler_notify(handle: &ElectionSchedulerHandle) {
    handle.0.notify()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_scheduler_len(handle: &ElectionSchedulerHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_scheduler_empty(handle: &ElectionSchedulerHandle) -> bool {
    handle.0.is_empty()
}
