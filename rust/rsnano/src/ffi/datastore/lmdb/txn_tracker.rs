use std::{ffi::c_void, sync::Arc, time::Duration};

use crate::{
    datastore::lmdb::TxnTracker,
    ffi::{FfiPropertyTreeWriter, LoggerHandle, LoggerMT, TxnTrackingConfigDto},
    DiagnosticsConfig,
};

pub struct MdbTxnTrackerHandle(Arc<TxnTracker>);

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_txn_tracker_create(
    logger: *mut LoggerHandle,
    config: *const TxnTrackingConfigDto,
    block_processor_batch_max_time_ms: u64,
) -> *mut MdbTxnTrackerHandle {
    let logger = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let config = DiagnosticsConfig::from(&*config);
    let block_processor_batch_max_time = Duration::from_millis(block_processor_batch_max_time_ms);
    Box::into_raw(Box::new(MdbTxnTrackerHandle(Arc::new(TxnTracker::new(
        logger,
        config.txn_tracking,
        block_processor_batch_max_time,
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_txn_tracker_destroy(handle: *mut MdbTxnTrackerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_txn_tracker_add(
    handle: *mut MdbTxnTrackerHandle,
    txn_id: u64,
    is_write: bool,
) {
    (*handle).0.add(txn_id, is_write);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_txn_tracker_erase(handle: *mut MdbTxnTrackerHandle, txn_id: u64) {
    (*handle).0.erase(txn_id);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_txn_tracker_serialize_json(
    handle: *mut MdbTxnTrackerHandle,
    json: *mut c_void,
    min_read_time_ms: u64,
    min_write_time_ms: u64,
) {
    let mut json = FfiPropertyTreeWriter::new_borrowed(json);
    (*handle)
        .0
        .serialize_json(
            &mut json,
            Duration::from_millis(min_read_time_ms),
            Duration::from_millis(min_write_time_ms),
        )
        .unwrap();
}
