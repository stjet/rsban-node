use std::sync::Arc;

use super::bootstrap_server::TcpServerHandle;
use crate::{
    ledger::datastore::LedgerHandle,
    messages::{downcast_message, MessageHandle},
    utils::{LoggerHandle, LoggerMT, ThreadPoolHandle},
    NodeConfigDto,
};
use rsnano_core::utils::Logger;
use rsnano_node::{bootstrap::FrontierReqServer, messages::FrontierReq};

pub struct FrontierReqServerHandle(FrontierReqServer);

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_server_create(
    tcp_server: *mut TcpServerHandle,
    request: *mut MessageHandle,
    thread_pool: *mut ThreadPoolHandle,
    logger: *mut LoggerHandle,
    config: *const NodeConfigDto,
    ledger: *mut LedgerHandle,
) -> *mut FrontierReqServerHandle {
    let logger: Arc<dyn Logger> = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    Box::into_raw(Box::new(FrontierReqServerHandle(FrontierReqServer::new(
        (*tcp_server).0.clone(),
        downcast_message::<FrontierReq>(request).clone(),
        (*thread_pool).0.clone(),
        logger,
        (&*config).try_into().unwrap(),
        (*ledger).0.clone(),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_server_destroy(handle: *mut FrontierReqServerHandle) {
    drop(Box::from_raw(handle))
}
