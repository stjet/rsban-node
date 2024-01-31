use super::bootstrap_server::TcpServerHandle;
use crate::{
    ledger::datastore::{
        lmdb::{PendingInfoDto, PendingKeyDto},
        LedgerHandle,
    },
    messages::MessageHandle,
    utils::{LoggerHandleV2, ThreadPoolHandle},
};
use rsnano_messages::Message;
use rsnano_node::bootstrap::BulkPullAccountServer;

pub struct BulkPullAccountServerHandle(BulkPullAccountServer);

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_account_server_create(
    request: &MessageHandle,
    server: *mut TcpServerHandle,
    ledger: *mut LedgerHandle,
    logger: &LoggerHandleV2,
    thread_pool: *mut ThreadPoolHandle,
) -> *mut BulkPullAccountServerHandle {
    let Message::BulkPullAccount(payload) = &request.message else {
        panic!("not a bulk_pull_account message")
    };
    let logger = logger.into_logger();
    Box::into_raw(Box::new(BulkPullAccountServerHandle(
        BulkPullAccountServer::new(
            (*server).0.clone(),
            payload.clone(),
            logger,
            (*thread_pool).0.clone(),
            (*ledger).0.clone(),
        ),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_account_server_destroy(
    handle: *mut BulkPullAccountServerHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_account_server_send_frontier(
    handle: *mut BulkPullAccountServerHandle,
) {
    (*handle).0.send_frontier();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_account_server_get_next(
    handle: *mut BulkPullAccountServerHandle,
    key: *mut PendingKeyDto,
    info: *mut PendingInfoDto,
) -> bool {
    if let Some((k, i)) = (*handle).0.get_next() {
        *key = PendingKeyDto::from(k);
        *info = i.into();
        true
    } else {
        false
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_account_server_current_key(
    handle: *mut BulkPullAccountServerHandle,
    key: *mut PendingKeyDto,
) {
    let k = (*handle).0.current_key();
    *key = PendingKeyDto::from(k);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_account_server_pending_address_only(
    handle: *mut BulkPullAccountServerHandle,
) -> bool {
    (*handle).0.pending_address_only()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_account_server_pending_include_address(
    handle: *mut BulkPullAccountServerHandle,
) -> bool {
    (*handle).0.pending_include_address()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_pull_account_server_invalid_request(
    handle: *mut BulkPullAccountServerHandle,
) -> bool {
    (*handle).0.invalid_request()
}
