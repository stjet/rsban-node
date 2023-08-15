use std::sync::Arc;

use rsnano_core::{utils::Logger, work::WorkThresholds};
use rsnano_node::{
    bootstrap::BootstrapMessageVisitorImpl,
    config::Logging,
    messages::{BulkPull, BulkPullAccount, BulkPush, FrontierReq, MessageVisitor},
};

use crate::{
    block_processing::BlockProcessorHandle,
    ledger::datastore::LedgerHandle,
    messages::{downcast_message, MessageHandle},
    utils::{LoggerHandle, LoggerMT, ThreadPoolHandle},
    work::WorkThresholdsDto,
    LoggingDto, NodeFlagsHandle, StatHandle,
};

use super::{bootstrap_initiator::BootstrapInitiatorHandle, bootstrap_server::TcpServerHandle};

pub struct BootstrapMessageVisitorHandle(BootstrapMessageVisitorImpl);

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_message_visitor_create(
    server: *mut TcpServerHandle,
    ledger: *mut LedgerHandle,
    logger: *mut LoggerHandle,
    thread_pool: *mut ThreadPoolHandle,
    block_processor: *mut BlockProcessorHandle,
    bootstrap_initiator: *mut BootstrapInitiatorHandle,
    stats: *mut StatHandle,
    work_thresholds: *const WorkThresholdsDto,
    flags: *mut NodeFlagsHandle,
    logging_config: *const LoggingDto,
) -> *mut BootstrapMessageVisitorHandle {
    let logger: Arc<dyn Logger> = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let visitor = BootstrapMessageVisitorImpl {
        connection: (*server).0.clone(),
        ledger: (*ledger).0.clone(),
        logger,
        thread_pool: (*thread_pool).0.clone(),
        block_processor: Arc::clone(&*block_processor),
        bootstrap_initiator: Arc::clone(&*bootstrap_initiator),
        stats: Arc::clone(&*stats),
        work_thresholds: WorkThresholds::from(&*work_thresholds),
        flags: (*flags).0.lock().unwrap().clone(),
        processed: false,
        logging_config: Logging::from(&*logging_config),
    };
    Box::into_raw(Box::new(BootstrapMessageVisitorHandle(visitor)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_message_visitor_destory(
    handle: *mut BootstrapMessageVisitorHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_message_visitor_processed_get(
    handle: *const BootstrapMessageVisitorHandle,
) -> bool {
    (*handle).0.processed
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_message_visitor_processed_set(
    handle: *mut BootstrapMessageVisitorHandle,
    processed: bool,
) {
    (*handle).0.processed = processed;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_message_visitor_bulk_pull(
    handle: *mut BootstrapMessageVisitorHandle,
    message: *mut MessageHandle,
) {
    let bulk_pull = downcast_message::<BulkPull>(message);
    (*handle).0.bulk_pull(bulk_pull);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_message_visitor_bulk_pull_account(
    handle: *mut BootstrapMessageVisitorHandle,
    message: *mut MessageHandle,
) {
    let bulk_pull = downcast_message::<BulkPullAccount>(message);
    (*handle).0.bulk_pull_account(bulk_pull);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_message_visitor_bulk_push(
    handle: *mut BootstrapMessageVisitorHandle,
    message: *mut MessageHandle,
) {
    let bulk_push = downcast_message::<BulkPush>(message);
    (*handle).0.bulk_push(bulk_push);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_message_visitor_frontier_req(
    handle: *mut BootstrapMessageVisitorHandle,
    message: *mut MessageHandle,
) {
    let frontier_req = downcast_message::<FrontierReq>(message);
    (*handle).0.frontier_req(frontier_req);
}
