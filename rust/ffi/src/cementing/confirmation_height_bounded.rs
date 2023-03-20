use std::{collections::HashMap, ffi::c_void, sync::Arc};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{Account, BlockHash};
use rsnano_node::{
    cementing::{
        truncate_after, ConfirmationHeightBounded, ConfirmedInfo, NotifyObserversCallback,
        WriteDetails,
    },
    config::Logging,
};

use crate::{
    copy_hash_bytes,
    core::BlockVecHandle,
    ledger::datastore::{
        LedgerHandle, TransactionHandle, WriteDatabaseQueueHandle, WriteGuardHandle,
    },
    utils::{ContextWrapper, LoggerHandle, LoggerMT, TimerHandle},
    LoggingDto, VoidPointerCallback,
};

use super::confirmation_height_unbounded::AtomicU64Handle;

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
) -> *mut ConfirmationHeightBoundedHandle {
    let notify_observers_context =
        ContextWrapper::new(notify_observers_context, notify_observers_drop_context);

    let notify_observers: NotifyObserversCallback = Box::new(move |blocks| {
        let cloned_blocks = blocks.clone();
        let block_vec_handle = Box::into_raw(Box::new(BlockVecHandle(cloned_blocks)));
        notify_observers_callback(notify_observers_context.get_context(), block_vec_handle);
    });

    let batch_write_size = Arc::clone(&(*batch_write_size).0);
    let logging = Logging::from(&*logging);

    Box::into_raw(Box::new(ConfirmationHeightBoundedHandle(
        ConfirmationHeightBounded::new(
            (*write_db_queue).0.clone(),
            notify_observers,
            batch_write_size,
            Arc::new(LoggerMT::new(Box::from_raw(logger))),
            logging,
            (*ledger).0.clone(),
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
    timer: *mut TimerHandle,
    txn: *mut TransactionHandle,
    cemented_blocks: *mut BlockVecHandle,
    write_guard: *mut WriteGuardHandle,
    amount_to_change: u64,
    error: *mut bool,
) -> *mut WriteGuardHandle {
    let (new_timer, write_guard) = (*handle).0.cement_blocks(
        (*timer).0,
        (*txn).as_write_txn(),
        &mut (*cemented_blocks).0,
        &mut (*write_guard).0,
        amount_to_change,
        &mut *error,
    );
    (*timer).0 = new_timer;

    match write_guard {
        Some(guard) => Box::into_raw(Box::new(WriteGuardHandle(guard))),
        None => std::ptr::null_mut(),
    }
}

// ----------------------------------
// PendingWritesQueue:

#[repr(C)]
pub struct WriteDetailsDto {
    pub account: [u8; 32],
    pub bottom_height: u64,
    pub bottom_hash: [u8; 32],
    pub top_height: u64,
    pub top_hash: [u8; 32],
}

impl From<&WriteDetailsDto> for WriteDetails {
    fn from(value: &WriteDetailsDto) -> Self {
        Self {
            account: Account::from_bytes(value.account),
            bottom_height: value.bottom_height,
            bottom_hash: BlockHash::from_bytes(value.bottom_hash),
            top_height: value.top_height,
            top_hash: BlockHash::from_bytes(value.top_hash),
        }
    }
}

impl From<&WriteDetails> for WriteDetailsDto {
    fn from(value: &WriteDetails) -> Self {
        Self {
            account: value.account.as_bytes().clone(),
            bottom_height: value.bottom_height,
            bottom_hash: value.bottom_hash.as_bytes().clone(),
            top_height: value.top_height,
            top_hash: value.top_hash.as_bytes().clone(),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_pending_writes_queue_size(
    handle: *mut ConfirmationHeightBoundedHandle,
) -> usize {
    (*handle).0.pending_writes.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_pending_writes_queue_push_back(
    handle: *mut ConfirmationHeightBoundedHandle,
    details: *const WriteDetailsDto,
) {
    (*handle)
        .0
        .pending_writes
        .push_back(WriteDetails::from(&*details))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_pending_writes_queue_front(
    handle: *mut ConfirmationHeightBoundedHandle,
    result: *mut WriteDetailsDto,
) {
    let details = (*handle).0.pending_writes.front().unwrap();
    (*result) = details.into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_pending_writes_queue_pop_front(
    handle: *mut ConfirmationHeightBoundedHandle,
) {
    (*handle).0.pending_writes.pop_front();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_pending_writes_queue_total_pending_write_block_count(
    handle: *mut ConfirmationHeightBoundedHandle,
) -> u64 {
    (*handle)
        .0
        .pending_writes
        .iter()
        .map(|i| i.top_height - i.bottom_height + 1)
        .sum()
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

pub struct AccountsConfirmedInfoHandle(pub HashMap<Account, ConfirmedInfo>);

#[no_mangle]
pub extern "C" fn rsn_accounts_confirmed_info_create() -> *mut AccountsConfirmedInfoHandle {
    Box::into_raw(Box::new(AccountsConfirmedInfoHandle(HashMap::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_accounts_confirmed_info_destroy(
    handle: *mut AccountsConfirmedInfoHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_accounts_confirmed_info_find(
    handle: *mut AccountsConfirmedInfoHandle,
    account: *const u8,
    result: *mut ConfirmedInfoDto,
) -> bool {
    match (*handle).0.get(&Account::from_ptr(account)) {
        Some(info) => {
            *result = info.into();
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_accounts_confirmed_info_size(
    handle: *mut AccountsConfirmedInfoHandle,
) -> usize {
    (*handle).0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_accounts_confirmed_info_insert(
    handle: *mut AccountsConfirmedInfoHandle,
    account: *const u8,
    info: *const ConfirmedInfoDto,
) {
    (*handle)
        .0
        .insert(Account::from_ptr(account), (&*info).into());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_accounts_confirmed_info_erase(
    handle: *mut AccountsConfirmedInfoHandle,
    account: *const u8,
) {
    (*handle).0.remove(&Account::from_ptr(account));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_accounts_confirmed_info_clear(
    handle: *mut AccountsConfirmedInfoHandle,
) {
    (*handle).0.clear();
}
