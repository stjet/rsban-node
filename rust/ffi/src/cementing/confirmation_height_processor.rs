use std::{
    ffi::c_void,
    ops::Deref,
    sync::{atomic::Ordering, Arc, Condvar, Mutex, MutexGuard, RwLock},
    time::Duration,
};

use num::FromPrimitive;
use rsnano_core::{BlockEnum, BlockHash};
use rsnano_node::{
    cementing::{ConfirmationHeightProcessor, GuardedData},
    config::Logging,
};

use crate::{
    copy_hash_bytes,
    core::{BlockCallback, BlockHandle, BlockHashCallback, BlockVecHandle},
    ledger::datastore::{LedgerHandle, WriteDatabaseQueueHandle, WriteGuardHandle},
    utils::{AtomicBoolHandle, AtomicU64Handle, ContextWrapper, LoggerHandle, LoggerMT},
    LoggingDto, StatHandle, VoidPointerCallback,
};

pub struct ConfirmationHeightProcessorHandle(ConfirmationHeightProcessor);

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_create(
    write_database_queue: *mut WriteDatabaseQueueHandle,
    logger: *mut LoggerHandle,
    logging: *const LoggingDto,
    ledger: *mut LedgerHandle,
    batch_separate_pending_min_time_ms: u64,
    stats: *mut StatHandle,
) -> *mut ConfirmationHeightProcessorHandle {
    let logger = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let logging = Logging::from(&*logging);

    Box::into_raw(Box::new(ConfirmationHeightProcessorHandle(
        ConfirmationHeightProcessor::new(
            (*write_database_queue).0.clone(),
            logger,
            logging,
            (*ledger).0.clone(),
            Duration::from_millis(batch_separate_pending_min_time_ms),
            (*stats).0.clone(),
        ),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_destroy(
    handle: *mut ConfirmationHeightProcessorHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_stopped(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> *mut AtomicBoolHandle {
    Box::into_raw(Box::new(AtomicBoolHandle((*handle).0.stopped.clone())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_pause(
    handle: *mut ConfirmationHeightProcessorHandle,
) {
    (*handle).0.pause();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_unpause(
    handle: *mut ConfirmationHeightProcessorHandle,
) {
    (*handle).0.unpause();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_add(
    handle: *mut ConfirmationHeightProcessorHandle,
    block: *const BlockHandle,
) {
    (*handle).0.add((*block).block.clone());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_set_next_hash(
    handle: *mut ConfirmationHeightProcessorHandle,
) {
    (*handle).0.set_next_hash();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_current(
    handle: *mut ConfirmationHeightProcessorHandle,
    hash: *mut u8,
) {
    let block_hash = (*handle).0.current();
    copy_hash_bytes(block_hash, hash);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_stop(
    handle: *mut ConfirmationHeightProcessorHandle,
) {
    (*handle).0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_run(
    handle: *mut ConfirmationHeightProcessorHandle,
    mode: u8,
) {
    let mode = FromPrimitive::from_u8(mode).unwrap();
    (*handle).0.run(mode);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_batch_write_size(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> *mut AtomicU64Handle {
    Box::into_raw(Box::new(AtomicU64Handle(
        (*handle).0.batch_write_size.clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_set_cemented_observer(
    handle: *mut ConfirmationHeightProcessorHandle,
    callback: BlockCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(context, delete_context);
    let callback_wrapper = Box::new(move |block: &Arc<RwLock<BlockEnum>>| {
        let block_handle = Box::into_raw(Box::new(BlockHandle::new(block.clone())));
        callback(context_wrapper.get_context(), block_handle);
        drop(Box::from_raw(block_handle));
    });
    (*handle).0.set_cemented_observer(callback_wrapper);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_clear_cemented_observer(
    handle: *mut ConfirmationHeightProcessorHandle,
) {
    (*handle).0.clear_cemented_observer();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_set_already_cemented_observer(
    handle: *mut ConfirmationHeightProcessorHandle,
    callback: BlockHashCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(context, delete_context);
    let callback_wrapper = Box::new(move |block_hash: BlockHash| {
        callback(
            context_wrapper.get_context(),
            block_hash.as_bytes().as_ptr(),
        );
    });
    (*handle).0.set_already_cemented_observer(callback_wrapper);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_is_processing_block(
    handle: *mut ConfirmationHeightProcessorHandle,
    block_hash: *const u8,
) -> bool {
    (*handle)
        .0
        .is_processing_block(&BlockHash::from_ptr(block_hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_is_processing_added_block(
    handle: *mut ConfirmationHeightProcessorHandle,
    block_hash: *const u8,
) -> bool {
    (*handle)
        .0
        .is_processing_added_block(&BlockHash::from_ptr(block_hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_unbounded_pending_writes(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> usize {
    (*handle)
        .0
        .unbounded_processor
        .pending_writes_size()
        .load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_awaiting_processing_size(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> usize {
    (*handle).0.awaiting_processing_len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_unbounded_pending_writes_size(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> usize {
    (*handle).0.unbounded_pending_writes_len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_bounded_pending_len(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> usize {
    (*handle)
        .0
        .bounded_processor
        .pending_writes_size
        .load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_bounded_accounts_confirmed_info_len(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> usize {
    (*handle)
        .0
        .bounded_processor
        .accounts_confirmed_info_size
        .load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_unbounded_conf_iterated_pairs_len(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> usize {
    (*handle)
        .0
        .unbounded_processor
        .confirmed_iterated_pairs_size_atomic()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_unbounded_implicit_receive_cemented_size(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> usize {
    (*handle)
        .0
        .unbounded_processor
        .implicit_receive_cemented_mapping_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_unbounded_block_cache_size(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> usize {
    (*handle).0.unbounded_processor.block_cache_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_awaiting_processing_entry_size() -> usize
{
    ConfirmationHeightProcessor::awaiting_processing_entry_size()
}
