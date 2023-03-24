use std::{
    ffi::c_void,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::BlockHash;
use rsnano_node::{
    cementing::{
        truncate_after, ConfirmationHeightBounded, ConfirmedInfo, NotifyObserversCallback,
    },
    config::Logging,
};

use crate::{
    copy_hash_bytes,
    core::{BlockHandle, BlockVecHandle},
    ledger::datastore::{LedgerHandle, WriteDatabaseQueueHandle, WriteGuardHandle},
    utils::{AtomicBoolHandle, AtomicU64Handle, ContextWrapper, LoggerHandle, LoggerMT},
    LoggingDto, VoidPointerCallback,
};

use super::confirmation_height_unbounded::{
    AwaitingProcessingSizeCallback, NotifyBlockAlreadyCementedCallback,
};

pub struct ConfirmationHeightBoundedHandle(ConfirmationHeightBounded);

pub type BlockVecCallback = extern "C" fn(*mut c_void, *mut BlockVecHandle);

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_create(
    write_db_queue: *mut WriteDatabaseQueueHandle,
    notify_observers_callback: BlockVecCallback,
    notify_observers_context: *mut c_void,
    notify_observers_drop_context: VoidPointerCallback,
    batch_write_size: *const AtomicU64Handle,
    logger: *mut LoggerHandle,
    logging: *const LoggingDto,
    ledger: *mut LedgerHandle,
    stopped: *mut AtomicBoolHandle,
    batch_separate_pending_min_time_ms: u64,
    awaiting_processing_size_callback: AwaitingProcessingSizeCallback,
    awaiting_processing_size_context: *mut c_void,
    awaiting_processing_size_context_delete: VoidPointerCallback,
    block_already_cemented_callback: NotifyBlockAlreadyCementedCallback,
    block_already_cemented_context: *mut c_void,
    block_already_cemented_context_delete: VoidPointerCallback,
) -> *mut ConfirmationHeightBoundedHandle {
    let notify_observers_context =
        ContextWrapper::new(notify_observers_context, notify_observers_drop_context);

    let notify_observers: NotifyObserversCallback = Box::new(move |blocks| {
        let cloned_blocks = blocks.clone();
        let block_vec_handle = Box::into_raw(Box::new(BlockVecHandle(cloned_blocks)));
        notify_observers_callback(notify_observers_context.get_context(), block_vec_handle);
    });

    let block_already_cemented_context = ContextWrapper::new(
        block_already_cemented_context,
        block_already_cemented_context_delete,
    );
    let block_already_cemented = Box::new(move |block_hash: BlockHash| {
        block_already_cemented_callback(
            block_already_cemented_context.get_context(),
            block_hash.as_bytes().as_ptr(),
        );
    });

    let batch_write_size = Arc::clone(&(*batch_write_size).0);
    let logging = Logging::from(&*logging);

    let context = ContextWrapper::new(
        awaiting_processing_size_context,
        awaiting_processing_size_context_delete,
    );
    let callback =
        Box::new(move || unsafe { awaiting_processing_size_callback(context.get_context()) });

    Box::into_raw(Box::new(ConfirmationHeightBoundedHandle(
        ConfirmationHeightBounded::new(
            (*write_db_queue).0.clone(),
            notify_observers,
            block_already_cemented,
            batch_write_size,
            Arc::new(LoggerMT::new(Box::from_raw(logger))),
            logging,
            (*ledger).0.clone(),
            (*stopped).0.clone(),
            Duration::from_millis(batch_separate_pending_min_time_ms),
            callback,
        ),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_destroy(
    handle: *mut ConfirmationHeightBoundedHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_cement_blocks(
    handle: *mut ConfirmationHeightBoundedHandle,
    write_guard: *mut WriteGuardHandle,
) -> *mut WriteGuardHandle {
    let write_guard = (*handle).0.cement_blocks(&mut (*write_guard).0);

    match write_guard {
        Some(guard) => Box::into_raw(Box::new(WriteGuardHandle(guard))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_accounts_confirmed_info_size(
    handle: *mut ConfirmationHeightBoundedHandle,
) -> usize {
    (*handle)
        .0
        .accounts_confirmed_info_size
        .load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_accounts_confirmed_info_size_store(
    handle: *mut ConfirmationHeightBoundedHandle,
    value: usize,
) {
    (*handle)
        .0
        .accounts_confirmed_info_size
        .store(value, Ordering::Relaxed);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_pending_writes_size(
    handle: *mut ConfirmationHeightBoundedHandle,
) -> usize {
    (*handle).0.pending_writes_size.load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_process(
    handle: *mut ConfirmationHeightBoundedHandle,
    original_block: *const BlockHandle,
) {
    (*handle)
        .0
        .process(&(*original_block).block.read().unwrap());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_clear_process_vars(
    handle: *mut ConfirmationHeightBoundedHandle,
) {
    (*handle).0.clear_process_vars();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_pending_empty(
    handle: *mut ConfirmationHeightBoundedHandle,
) -> bool {
    (*handle).0.pending_empty()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_write_details_size() -> usize {
    ConfirmationHeightBounded::write_details_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_confirmed_info_entry_size() -> usize {
    ConfirmationHeightBounded::confirmed_info_entry_size()
}

// ----------------------------------
// HashCircularBuffer:

pub struct HashCircularBufferHandle(BoundedVecDeque<BlockHash>);

#[no_mangle]
pub extern "C" fn rsn_hash_circular_buffer_create(
    max_size: usize,
) -> *mut HashCircularBufferHandle {
    Box::into_raw(Box::new(HashCircularBufferHandle(BoundedVecDeque::new(
        max_size,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hash_circular_buffer_destroy(handle: *mut HashCircularBufferHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hash_circular_buffer_push_back(
    handle: *mut HashCircularBufferHandle,
    hash: *const u8,
) {
    (*handle).0.push_back(BlockHash::from_ptr(hash));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hash_circular_buffer_empty(
    handle: *mut HashCircularBufferHandle,
) -> bool {
    (*handle).0.is_empty()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hash_circular_buffer_back(
    handle: *mut HashCircularBufferHandle,
    result: *mut u8,
) {
    let hash = (*handle).0.back().unwrap();
    copy_hash_bytes(*hash, result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_hash_circular_buffer_truncate_after(
    handle: *mut HashCircularBufferHandle,
    hash: *const u8,
) {
    truncate_after(&mut (*handle).0, &BlockHash::from_ptr(hash));
}

// ----------------------------------
// AccountsConfirmedInfo:

#[repr(C)]
pub struct ConfirmedInfoDto {
    pub confirmed_height: u64,
    pub iterated_frontier: [u8; 32],
}

impl From<&ConfirmedInfo> for ConfirmedInfoDto {
    fn from(value: &ConfirmedInfo) -> Self {
        Self {
            confirmed_height: value.confirmed_height,
            iterated_frontier: value.iterated_frontier.as_bytes().clone(),
        }
    }
}

impl From<&ConfirmedInfoDto> for ConfirmedInfo {
    fn from(value: &ConfirmedInfoDto) -> Self {
        Self {
            confirmed_height: value.confirmed_height,
            iterated_frontier: BlockHash::from_bytes(value.iterated_frontier),
        }
    }
}
