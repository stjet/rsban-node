use std::sync::Arc;

use super::bootstrap_server::TcpServerHandle;
use crate::{
    ledger::datastore::LedgerHandle,
    messages::MessageHandle,
    utils::{LoggerHandle, LoggerMT, ThreadPoolHandle},
    NodeConfigDto,
};
use rsnano_core::utils::Logger;
use rsnano_node::{bootstrap::FrontierReqServer, config::NodeConfig, messages::Message};

pub struct FrontierReqServerHandle(FrontierReqServer);

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_server_create(
    tcp_server: *mut TcpServerHandle,
    request: &MessageHandle,
    thread_pool: *mut ThreadPoolHandle,
    logger: *mut LoggerHandle,
    config: *const NodeConfigDto,
    ledger: *mut LedgerHandle,
) -> *mut FrontierReqServerHandle {
    let logger: Arc<dyn Logger> = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let config: NodeConfig = (&*config).try_into().unwrap();
    let Message::FrontierReq(request) = &request.message else {
        panic!("not a frontier_req")
    };
    Box::into_raw(Box::new(FrontierReqServerHandle(FrontierReqServer::new(
        (*tcp_server).0.clone(),
        request.clone(),
        (*thread_pool).0.clone(),
        logger,
        config.logging.bulk_pull_logging(),
        config.logging.network_logging_value,
        (*ledger).0.clone(),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_server_destroy(handle: *mut FrontierReqServerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_server_send_next(handle: *mut FrontierReqServerHandle) {
    (*handle).0.send_next()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_server_current(
    handle: *mut FrontierReqServerHandle,
    current: *mut u8,
) {
    (*handle).0.current().copy_bytes(current);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_server_frontier(
    handle: *mut FrontierReqServerHandle,
    frontier: *mut u8,
) {
    (*handle).0.frontier().copy_bytes(frontier);
}
