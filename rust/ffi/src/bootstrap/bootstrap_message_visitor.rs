use std::sync::Arc;

use rsnano_core::{utils::Logger, work::WorkThresholds};
use rsnano_node::bootstrap::BootstrapMessageVisitorImpl;

use crate::{
    block_processing::BlockProcessorHandle,
    ledger::datastore::LedgerHandle,
    utils::{LoggerHandle, LoggerMT, ThreadPoolHandle},
    work::WorkThresholdsDto,
    NodeFlagsHandle, StatHandle,
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
        flags: Arc::clone(&(*flags).0),
        processed: false,
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
