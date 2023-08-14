use std::sync::Arc;

use rsnano_core::{utils::Logger, work::WorkThresholds};
use rsnano_node::bootstrap::BulkPushServer;

use crate::{
    block_processing::BlockProcessorHandle,
    ledger::datastore::LedgerHandle,
    utils::{LoggerHandle, LoggerMT, ThreadPoolHandle},
    work::WorkThresholdsDto,
    StatHandle,
};

use super::{bootstrap_initiator::BootstrapInitiatorHandle, bootstrap_server::TcpServerHandle};

pub struct BulkPushServerHandle(BulkPushServer);

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_push_server_create(
    server: *mut TcpServerHandle,
    ledger: *mut LedgerHandle,
    logger: *mut LoggerHandle,
    thread_pool: *mut ThreadPoolHandle,
    logging_enabled: bool,
    network_logging_enabled: bool,
    block_processor: *mut BlockProcessorHandle,
    bootstrap_initiator: *mut BootstrapInitiatorHandle,
    stats: *mut StatHandle,
    work_thresholds: *const WorkThresholdsDto,
) -> *mut BulkPushServerHandle {
    let logger: Arc<dyn Logger> = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    Box::into_raw(Box::new(BulkPushServerHandle(BulkPushServer::new(
        (*server).0.clone(),
        (*ledger).0.clone(),
        logger,
        (*thread_pool).0.clone(),
        logging_enabled,
        network_logging_enabled,
        Arc::clone(&*block_processor),
        Arc::clone(&*bootstrap_initiator),
        Arc::clone(&*stats),
        WorkThresholds::from(&*work_thresholds),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bulk_push_server_destroy(handle: *mut BulkPushServerHandle) {
    drop(Box::from_raw(handle))
}
