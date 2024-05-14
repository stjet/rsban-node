use super::{
    vote_generator::VoteGeneratorHandle, ActiveTransactionsHandle, LocalVoteHistoryHandle,
};
use crate::{
    ledger::datastore::LedgerHandle, transport::ChannelHandle, utils::ContainerInfoComponentHandle,
    wallets::LmdbWalletsHandle, NodeConfigDto, StatHandle,
};
use rsnano_core::{BlockHash, Root};
use rsnano_node::consensus::{RequestAggregator, RequestAggregatorExt};
use std::{
    ffi::{c_char, CStr},
    ops::Deref,
    sync::Arc,
};

pub struct RequestAggregatorHandle(Arc<RequestAggregator>);

impl Deref for RequestAggregatorHandle {
    type Target = Arc<RequestAggregator>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_request_aggregator_create(
    config: &NodeConfigDto,
    stats: &StatHandle,
    generator: &VoteGeneratorHandle,
    final_generator: &VoteGeneratorHandle,
    local_votes: &LocalVoteHistoryHandle,
    ledger: &LedgerHandle,
    wallets: &LmdbWalletsHandle,
    active: &ActiveTransactionsHandle,
    is_dev_network: bool,
) -> *mut RequestAggregatorHandle {
    Box::into_raw(Box::new(RequestAggregatorHandle(Arc::new(
        RequestAggregator::new(
            config.try_into().unwrap(),
            Arc::clone(&stats),
            Arc::clone(&generator),
            Arc::clone(&final_generator),
            Arc::clone(&local_votes),
            Arc::clone(&ledger),
            Arc::clone(&wallets),
            Arc::clone(&active),
            is_dev_network,
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_request_aggregator_destroy(handle: *mut RequestAggregatorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_request_aggregator_start(handle: &RequestAggregatorHandle) {
    handle.0.start();
}

#[no_mangle]
pub extern "C" fn rsn_request_aggregator_add(
    handle: &RequestAggregatorHandle,
    channel: &ChannelHandle,
    hashes_roots: &HashesRootsVecHandle,
) {
    handle.0.add(Arc::clone(channel), &hashes_roots.0);
}

#[no_mangle]
pub extern "C" fn rsn_request_aggregator_stop(handle: &RequestAggregatorHandle) {
    handle.0.stop();
}

#[no_mangle]
pub extern "C" fn rsn_request_aggregator_len(handle: &RequestAggregatorHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub extern "C" fn rsn_request_aggregator_max_delay_ms(handle: &RequestAggregatorHandle) -> u64 {
    handle.0.max_delay.as_millis() as u64
}

#[no_mangle]
pub unsafe extern "C" fn rsn_request_aggregator_collect_container_info(
    handle: &RequestAggregatorHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = handle
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}

pub struct HashesRootsVecHandle(Vec<(BlockHash, Root)>);

#[no_mangle]
pub extern "C" fn rsn_hashes_roots_vec_create() -> *mut HashesRootsVecHandle {
    Box::into_raw(Box::new(HashesRootsVecHandle(Vec::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hashes_roots_vec_destroy(handle: *mut HashesRootsVecHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hashes_roots_vec_push(
    handle: &mut HashesRootsVecHandle,
    hash: *const u8,
    root: *const u8,
) {
    handle
        .0
        .push((BlockHash::from_ptr(hash), Root::from_ptr(root)))
}
