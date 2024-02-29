use super::{
    bootstrap_attempt::BootstrapAttemptHandle, bootstrap_client::BootstrapClientHandle,
    bootstrap_connections::BootstrapConnectionsHandle,
};
use crate::{ledger::datastore::LedgerHandle, NetworkParamsDto};
use rsnano_core::Account;
use rsnano_node::bootstrap::{FrontierReqClient, FrontierReqClientExt};
use std::sync::Arc;

pub struct FrontierReqClientHandle(Arc<FrontierReqClient>);

#[no_mangle]
pub extern "C" fn rsn_frontier_req_client_create(
    connection: &BootstrapClientHandle,
    ledger: &LedgerHandle,
    network_params: &NetworkParamsDto,
    connections: &BootstrapConnectionsHandle,
    attempt: &BootstrapAttemptHandle,
) -> *mut FrontierReqClientHandle {
    let network_params = network_params.try_into().unwrap();
    let mut client = FrontierReqClient::new(
        Arc::clone(connection),
        Arc::clone(ledger),
        network_params,
        Arc::clone(connections),
    );
    client.set_attempt(Arc::clone(attempt));
    Box::into_raw(Box::new(FrontierReqClientHandle(Arc::new(client))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_client_destroy(handle: *mut FrontierReqClientHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_client_run(
    handle: &FrontierReqClientHandle,
    start_account: *const u8,
    frontiers_age: u32,
    count: u32,
) {
    handle
        .0
        .run(&Account::from_ptr(start_account), frontiers_age, count);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_client_get_result(
    handle: &FrontierReqClientHandle,
) -> bool {
    handle.0.get_result()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_client_set_result(
    handle: &FrontierReqClientHandle,
    result: bool,
) {
    handle.0.set_result(result)
}
