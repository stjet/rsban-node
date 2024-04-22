use super::{OnlineRepsHandle, RepresentativeRegisterHandle};
use crate::{
    consensus::ActiveTransactionsHandle, ledger::datastore::LedgerHandle,
    transport::TcpChannelsHandle, utils::AsyncRuntimeHandle, NetworkParamsDto, NodeConfigDto,
    StatHandle,
};
use rsnano_node::representatives::RepCrawler;
use std::{
    ffi::{c_char, CStr},
    ops::Deref,
    sync::Arc,
    time::Duration,
};

pub struct RepCrawlerHandle(Arc<RepCrawler>);

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
pub unsafe extern "C" fn rsn_rep_crawler_keepalive(
    handle: &RepCrawlerHandle,
    address: *const c_char,
    port: u16,
) {
    let address = CStr::from_ptr(address).to_str().unwrap().to_string();
    handle.keepalive_or_connect(address, port);
}
