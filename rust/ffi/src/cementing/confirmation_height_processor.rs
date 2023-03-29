use std::{
    ffi::{c_char, c_void, CStr},
    ops::Deref,
    sync::{Arc, RwLock},
    time::Duration,
};

use num::FromPrimitive;
use rsnano_core::{BlockEnum, BlockHash};
use rsnano_node::{cementing::ConfirmationHeightProcessor, config::Logging};

use crate::{
    copy_hash_bytes,
    core::{BlockCallback, BlockHandle, BlockHashCallback},
    ledger::datastore::{LedgerHandle, WriteDatabaseQueueHandle},
    utils::{ContainerInfoComponentHandle, ContextWrapper, FfiLatch, LoggerHandle, LoggerMT},
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
    latch: *mut c_void,
    mode: u8,
) -> *mut ConfirmationHeightProcessorHandle {
    let logger = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let logging = Logging::from(&*logging);
    let latch = Box::new(FfiLatch::new(latch));

    Box::into_raw(Box::new(ConfirmationHeightProcessorHandle(
        ConfirmationHeightProcessor::new(
            (*write_database_queue).0.clone(),
            logger,
            logging.timing_logging_value,
            (*ledger).0.clone(),
            Duration::from_millis(batch_separate_pending_min_time_ms),
            (*stats).0.clone(),
            latch,
            FromPrimitive::from_u8(mode).unwrap(),
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
    (*handle)
        .0
        .add(Arc::new((*block).block.read().unwrap().clone()));
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
pub unsafe extern "C" fn rsn_confirmation_height_processor_set_cemented_observer(
    handle: *mut ConfirmationHeightProcessorHandle,
    callback: BlockCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(context, delete_context);
    let callback_wrapper = Box::new(move |block: &Arc<BlockEnum>| {
        let block_handle = Box::into_raw(Box::new(BlockHandle::new(Arc::new(RwLock::new(
            block.deref().clone(),
        )))));
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
pub unsafe extern "C" fn rsn_confirmation_height_processor_awaiting_processing_size(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> usize {
    (*handle).0.awaiting_processing_len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_set_batch_write_size(
    handle: *mut ConfirmationHeightProcessorHandle,
    size: usize,
) {
    (*handle).0.set_batch_write_size(size);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_collect_container_info(
    handle: *const ConfirmationHeightProcessorHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = (*handle)
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
