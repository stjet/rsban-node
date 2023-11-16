use crate::{
    ledger::datastore::{LedgerHandle, TransactionHandle},
    StatHandle,
};
use rsnano_core::{BlockHash, Root};
use rsnano_node::consensus::VoteGenerator;
use std::{ops::Deref, sync::Arc};

pub struct VoteGeneratorHandle(VoteGenerator);

impl Deref for VoteGeneratorHandle {
    type Target = VoteGenerator;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_generator_create(
    ledger: &LedgerHandle,
    is_final: bool,
    stats: &StatHandle,
) -> *mut VoteGeneratorHandle {
    Box::into_raw(Box::new(VoteGeneratorHandle(VoteGenerator::new(
        Arc::clone(ledger),
        is_final,
        Arc::clone(stats),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_generator_destroy(handle: *mut VoteGeneratorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_generator_should_vote(
    handle: &VoteGeneratorHandle,
    transaction: &mut TransactionHandle,
    root: *const u8,
    hash: *const u8,
) -> bool {
    let root = Root::from_ptr(root);
    let hash = BlockHash::from_ptr(hash);
    handle.should_vote(transaction.as_write_txn(), &root, &hash)
}
