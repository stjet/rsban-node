use rsnano_node::bootstrap::BulkPullServer;

pub struct BulkPullServerHandle(BulkPullServer);

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_server_create() -> *mut BulkPullServerHandle {
    Box::into_raw(Box::new(BulkPullServerHandle(BulkPullServer::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_server_destroy(handle: *mut BulkPullServerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_server_sent_count(
    handle: *const BulkPullServerHandle,
) -> u32 {
    (*handle).0.sent_count
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_server_sent_count_set(
    handle: *mut BulkPullServerHandle,
    value: u32,
) {
    (*handle).0.sent_count = value;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_server_max_count(
    handle: *const BulkPullServerHandle,
) -> u32 {
    (*handle).0.max_count
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_server_max_count_set(
    handle: *mut BulkPullServerHandle,
    value: u32,
) {
    (*handle).0.max_count = value;
}
