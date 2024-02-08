use super::bootstrap_server::TcpServerHandle;
use crate::{ledger::datastore::LedgerHandle, messages::MessageHandle, utils::ThreadPoolHandle};
use rsnano_messages::Message;
use rsnano_node::bootstrap::FrontierReqServer;

pub struct FrontierReqServerHandle(FrontierReqServer);

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_server_create(
    tcp_server: *mut TcpServerHandle,
    request: &MessageHandle,
    thread_pool: *mut ThreadPoolHandle,
    ledger: *mut LedgerHandle,
) -> *mut FrontierReqServerHandle {
    let Message::FrontierReq(request) = &request.message else {
        panic!("not a frontier_req")
    };
    Box::into_raw(Box::new(FrontierReqServerHandle(FrontierReqServer::new(
        (*tcp_server).0.clone(),
        request.clone(),
        (*thread_pool).0.clone(),
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
