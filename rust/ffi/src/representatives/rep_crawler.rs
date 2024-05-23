use super::{OnlineRepsHandle, RepresentativeRegisterHandle};
use crate::{
    consensus::{ActiveTransactionsHandle, VoteHandle},
    ledger::datastore::LedgerHandle,
    transport::{ChannelHandle, TcpChannelsHandle},
    utils::AsyncRuntimeHandle,
    NetworkParamsDto, NodeConfigDto, StatHandle,
};
use rsnano_core::BlockHash;
use rsnano_node::representatives::{RepCrawler, RepCrawlerExt};
use std::{
    ffi::{c_char, CStr},
    ops::Deref,
    sync::Arc,
    time::Duration,
};

pub struct RepCrawlerHandle(pub Arc<RepCrawler>);

impl Deref for RepCrawlerHandle {
    type Target = Arc<RepCrawler>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_rep_crawler_create(
    representative_register: &RepresentativeRegisterHandle,
    stats: &StatHandle,
    query_timeout_ms: u64,
    online_reps: &OnlineRepsHandle,
    config: &NodeConfigDto,
    network_params: &NetworkParamsDto,
    tcp_channels: &TcpChannelsHandle,
    async_rt: &AsyncRuntimeHandle,
    ledger: &LedgerHandle,
    active: &ActiveTransactionsHandle,
) -> *mut RepCrawlerHandle {
    Box::into_raw(Box::new(RepCrawlerHandle(Arc::new(RepCrawler::new(
        Arc::clone(representative_register),
        Arc::clone(stats),
        Duration::from_millis(query_timeout_ms),
        Arc::clone(online_reps),
        config.try_into().unwrap(),
        network_params.try_into().unwrap(),
        Arc::clone(tcp_channels),
        Arc::clone(async_rt),
        Arc::clone(ledger),
        Arc::clone(active),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_destroy(handle: *mut RepCrawlerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_rep_crawler_start(handle: &RepCrawlerHandle) {
    handle.start();
}

#[no_mangle]
pub extern "C" fn rsn_rep_crawler_stop(handle: &RepCrawlerHandle) {
    handle.stop();
}

#[no_mangle]
pub extern "C" fn rsn_rep_crawler_process(
    handle: &RepCrawlerHandle,
    vote: &VoteHandle,
    channel: &ChannelHandle,
) -> bool {
    handle.process(Arc::clone(vote), Arc::clone(channel))
}

#[no_mangle]
pub extern "C" fn rsn_rep_crawler_query(handle: &RepCrawlerHandle, channel: &ChannelHandle) {
    handle.query_channel(Arc::clone(channel))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_keepalive(
    handle: &RepCrawlerHandle,
    address: *const c_char,
    port: u16,
) {
    let address = CStr::from_ptr(address).to_str().unwrap().to_string();
    handle.keepalive_or_connect(address, port);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_force_process(
    handle: &RepCrawlerHandle,
    vote: &VoteHandle,
    channel: &ChannelHandle,
) {
    handle.force_process(Arc::clone(vote), Arc::clone(channel))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_force_query(
    handle: &RepCrawlerHandle,
    hash: *const u8,
    channel: &ChannelHandle,
) {
    handle.force_query(BlockHash::from_ptr(hash), Arc::clone(channel))
}
