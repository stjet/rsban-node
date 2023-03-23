use std::{
    ffi::c_void,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{Account, BlockHash};
use rsnano_node::{
    cementing::{
        truncate_after, ConfirmationHeightBounded, ConfirmedInfo, NotifyObserversCallback,
        ReceiveChainDetails, ReceiveSourcePair, TopAndNextHash, WriteDetails,
    },
    config::Logging,
};

use crate::{
    copy_hash_bytes,
    core::{BlockHandle, BlockVecHandle},
    ledger::datastore::{
        LedgerHandle, TransactionHandle, WriteDatabaseQueueHandle, WriteGuardHandle,
    },
    utils::{
        AtomicBoolHandle, AtomicU64Handle, ContextWrapper, LoggerHandle, LoggerMT, TimerHandle,
    },
    ConfirmationHeightInfoDto, LoggingDto, VoidPointerCallback,
};

use super::confirmation_height_unbounded::AwaitingProcessingSizeCallback;

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
    timer: *mut TimerHandle,
    batch_separate_pending_min_time_ms: u64,
    awaiting_processing_size_callback: AwaitingProcessingSizeCallback,
    awaiting_processing_size_context: *mut c_void,
    awaiting_processing_size_context_delete: VoidPointerCallback,
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
            batch_write_size,
            Arc::new(LoggerMT::new(Box::from_raw(logger))),
            logging,
            (*ledger).0.clone(),
            (*stopped).0.clone(),
            (*timer).0.clone(),
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
pub unsafe extern "C" fn rsn_confirmation_height_bounded_prepare_iterated_blocks_for_cementing(
    handle: *mut ConfirmationHeightBoundedHandle,
    has_details: bool,
    details: *const ReceiveChainDetailsDto,
    checkpoints: *mut HashCircularBufferHandle,
    has_next_in_receive_chain: *mut bool,
    next_in_receive_chain: *mut TopAndNextHashDto,
    already_cemented: bool,
    txn: *mut TransactionHandle,
    top_most_non_receive_block_hash: *const u8,
    conf_height_info: *const ConfirmationHeightInfoDto,
    account: *const u8,
    bottom_height: u64,
    bottom_most: *const u8,
) {
    let mut next = if *has_next_in_receive_chain {
        Some((&*next_in_receive_chain).into())
    } else {
        None
    };

    let details = if has_details {
        Some((&*details).into())
    } else {
        None
    };

    let txn = (*txn).as_txn();
    let conf_height_info = (&*conf_height_info).into();

    (*handle).0.prepare_iterated_blocks_for_cementing(
        &details,
        &mut (*checkpoints).0,
        &mut next,
        already_cemented,
        txn,
        &BlockHash::from_ptr(top_most_non_receive_block_hash),
        &conf_height_info,
        &Account::from_ptr(account),
        bottom_height,
        &BlockHash::from_ptr(bottom_most),
    );

    *has_next_in_receive_chain = next.is_some();
    if let Some(next) = &next {
        *next_in_receive_chain = next.into();
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_iterate(
    handle: *mut ConfirmationHeightBoundedHandle,
    receive_source_pairs: *mut ReceiveSourcePairCircularBufferHandle,
    checkpoints: *mut HashCircularBufferHandle,
    top_level_hash: *const u8,
    account: *const u8,
    bottom_height: u64,
    bottom_hash: *const u8,
    top_most_non_receive_block_hash: *mut u8,
    txn: *mut TransactionHandle,
) -> bool {
    let mut top_most_receive = BlockHash::from_ptr(top_most_non_receive_block_hash);
    let hit_receive = (*handle).0.iterate(
        &mut (*receive_source_pairs).0,
        &mut (*checkpoints).0,
        BlockHash::from_ptr(top_level_hash),
        Account::from_ptr(account),
        bottom_height,
        BlockHash::from_ptr(bottom_hash),
        &mut top_most_receive,
        (*txn).as_read_txn_mut(),
    );

    copy_hash_bytes(top_most_receive, top_most_non_receive_block_hash);
    hit_receive
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_get_least_unconfirmed_hash_from_top_level(
    handle: *mut ConfirmationHeightBoundedHandle,
    txn: *const TransactionHandle,
    hash: *const u8,
    account: *const u8,
    conf_height_info: *const ConfirmationHeightInfoDto,
    block_height: *mut u64,
    least_confirmed_hash: *mut u8,
) {
    let conf_height = (&*conf_height_info).into();
    let least_confirmed = (*handle).0.get_least_confirmed_hash_from_top_level(
        (*txn).as_txn(),
        &BlockHash::from_ptr(hash),
        &Account::from_ptr(account),
        &conf_height,
        &mut *block_height,
    );
    copy_hash_bytes(least_confirmed, least_confirmed_hash);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_get_next_block(
    handle: *mut ConfirmationHeightBoundedHandle,
    next_in_receive_chain: *const TopAndNextHashDto,
    has_next_in_receive_chain: bool,
    checkpoints: *const HashCircularBufferHandle,
    receive_source_pairs: *const ReceiveSourcePairCircularBufferHandle,
    receive_details: *mut ReceiveChainDetailsDto,
    has_receive_details: *mut bool,
    original_block: *const BlockHandle,
    next: *mut TopAndNextHashDto,
) {
    let next_in_receive_chain = if has_next_in_receive_chain {
        Some((&*next_in_receive_chain).into())
    } else {
        None
    };

    let mut receive_details_copy = if *has_receive_details {
        Some((&*receive_details).into())
    } else {
        None
    };

    let next_block = (*handle).0.get_next_block(
        &next_in_receive_chain,
        &(*checkpoints).0,
        &(*receive_source_pairs).0,
        &mut receive_details_copy,
        &(*original_block).block.read().unwrap(),
    );

    *has_receive_details = receive_details_copy.is_some();
    if let Some(details) = &receive_details_copy {
        *receive_details = details.into();
    }

    *next = (&next_block).into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_bounded_process(
    handle: *mut ConfirmationHeightBoundedHandle,
    current: *const u8,
    original_block: *const BlockHandle,
) {
    (*handle).0.process(
        &BlockHash::from_ptr(current),
        &(*original_block).block.read().unwrap(),
    );
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
// ReceiveSourcePairCircularBuffer:

#[repr(C)]
pub struct ReceiveSourcePairDto {
    pub receive_details: ReceiveChainDetailsDto,
    pub source_hash: [u8; 32],
}

impl From<&ReceiveSourcePair> for ReceiveSourcePairDto {
    fn from(value: &ReceiveSourcePair) -> Self {
        Self {
            receive_details: (&value.receive_details).into(),
            source_hash: value.source_hash.as_bytes().clone(),
        }
    }
}

impl From<&ReceiveSourcePairDto> for ReceiveSourcePair {
    fn from(value: &ReceiveSourcePairDto) -> Self {
        Self {
            receive_details: (&value.receive_details).into(),
            source_hash: BlockHash::from_bytes(value.source_hash),
        }
    }
}

pub struct ReceiveSourcePairCircularBufferHandle(BoundedVecDeque<ReceiveSourcePair>);

#[no_mangle]
pub extern "C" fn rsn_receive_source_pair_circular_buffer_create(
    max_size: usize,
) -> *mut ReceiveSourcePairCircularBufferHandle {
    Box::into_raw(Box::new(ReceiveSourcePairCircularBufferHandle(
        BoundedVecDeque::new(max_size),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_circular_buffer_destroy(
    handle: *mut ReceiveSourcePairCircularBufferHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_circular_buffer_push_back(
    handle: *mut ReceiveSourcePairCircularBufferHandle,
    item: *const ReceiveSourcePairDto,
) {
    (*handle).0.push_back((&*item).into());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_circular_buffer_size(
    handle: *mut ReceiveSourcePairCircularBufferHandle,
) -> usize {
    (*handle).0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_circular_buffer_back(
    handle: *mut ReceiveSourcePairCircularBufferHandle,
    result: *mut ReceiveSourcePairDto,
) {
    *result = (*handle).0.back().unwrap().into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_circular_buffer_pop_back(
    handle: *mut ReceiveSourcePairCircularBufferHandle,
) {
    (*handle).0.pop_back();
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

#[no_mangle]
pub unsafe extern "C" fn rsn_accounts_confirmed_info_find(
    handle: *mut ConfirmationHeightBoundedHandle,
    account: *const u8,
    result: *mut ConfirmedInfoDto,
) -> bool {
    match (*handle)
        .0
        .accounts_confirmed_info
        .get(&Account::from_ptr(account))
    {
        Some(info) => {
            *result = info.into();
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_accounts_confirmed_info_size(
    handle: *mut ConfirmationHeightBoundedHandle,
) -> usize {
    (*handle).0.accounts_confirmed_info.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_accounts_confirmed_info_insert(
    handle: *mut ConfirmationHeightBoundedHandle,
    account: *const u8,
    info: *const ConfirmedInfoDto,
) {
    (*handle)
        .0
        .accounts_confirmed_info
        .insert(Account::from_ptr(account), (&*info).into());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_accounts_confirmed_info_erase(
    handle: *mut ConfirmationHeightBoundedHandle,
    account: *const u8,
) {
    (*handle)
        .0
        .accounts_confirmed_info
        .remove(&Account::from_ptr(account));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_accounts_confirmed_info_clear(
    handle: *mut ConfirmationHeightBoundedHandle,
) {
    (*handle).0.accounts_confirmed_info.clear();
}

// ----------------------------------
// ReceiveChainDetails:

#[repr(C)]
pub struct ReceiveChainDetailsDto {
    pub account: [u8; 32],
    pub height: u64,
    pub hash: [u8; 32],
    pub top_level: [u8; 32],
    pub next: [u8; 32],
    pub has_next: bool,
    pub bottom_height: u64,
    pub bottom_most: [u8; 32],
}

impl From<&ReceiveChainDetails> for ReceiveChainDetailsDto {
    fn from(value: &ReceiveChainDetails) -> Self {
        Self {
            account: value.account.as_bytes().clone(),
            height: value.height,
            hash: value.hash.as_bytes().clone(),
            top_level: value.top_level.as_bytes().clone(),
            next: value.next.unwrap_or_default().as_bytes().clone(),
            has_next: value.next.is_some(),
            bottom_height: value.bottom_height,
            bottom_most: value.bottom_most.as_bytes().clone(),
        }
    }
}

impl From<&ReceiveChainDetailsDto> for ReceiveChainDetails {
    fn from(value: &ReceiveChainDetailsDto) -> Self {
        Self {
            account: Account::from_bytes(value.account),
            height: value.height,
            hash: BlockHash::from_bytes(value.hash),
            top_level: BlockHash::from_bytes(value.top_level),
            next: if value.has_next {
                Some(BlockHash::from_bytes(value.next))
            } else {
                None
            },
            bottom_height: value.bottom_height,
            bottom_most: BlockHash::from_bytes(value.bottom_most),
        }
    }
}

#[repr(C)]
pub struct TopAndNextHashDto {
    pub top: [u8; 32],
    pub has_next: bool,
    pub next: [u8; 32],
    pub next_height: u64,
}

impl From<&TopAndNextHash> for TopAndNextHashDto {
    fn from(value: &TopAndNextHash) -> Self {
        Self {
            top: value.top.as_bytes().clone(),
            has_next: value.next.is_some(),
            next: value.next.unwrap_or_default().as_bytes().clone(),
            next_height: value.next_height,
        }
    }
}

impl From<&TopAndNextHashDto> for TopAndNextHash {
    fn from(value: &TopAndNextHashDto) -> Self {
        Self {
            top: BlockHash::from_bytes(value.top),
            next: if value.has_next {
                Some(BlockHash::from_bytes(value.next))
            } else {
                None
            },
            next_height: value.next_height,
        }
    }
}
