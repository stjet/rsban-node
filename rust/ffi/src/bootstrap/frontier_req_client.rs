use std::sync::Arc;

use rsnano_node::bootstrap::FrontierReqClient;

pub struct FrontierReqClientHandle(Arc<FrontierReqClient>);

#[no_mangle]
pub extern "C" fn rsn_frontier_req_client_create() -> *mut FrontierReqClientHandle {
    Box::into_raw(Box::new(FrontierReqClientHandle(Arc::new(
        FrontierReqClient::new(),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_frontier_req_client_destroy(handle: *mut FrontierReqClientHandle) {
    drop(Box::from_raw(handle))
}
