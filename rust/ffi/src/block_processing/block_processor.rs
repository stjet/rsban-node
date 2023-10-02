use crate::{
    core::{BlockHandle, EpochsHandle},
    gap_cache::GapCacheHandle,
    ledger::datastore::{LedgerHandle, TransactionHandle, WriteDatabaseQueueHandle},
    unchecked_map::UncheckedMapHandle,
    utils::{ContainerInfoComponentHandle, LoggerHandle, LoggerMT},
    voting::LocalVoteHistoryHandle,
    work::WorkThresholdsDto,
    NodeConfigDto, NodeFlagsHandle, SignatureCheckerHandle, StatHandle,
};
use rsnano_core::{work::WorkThresholds, BlockEnum, HashOrAccount};
use rsnano_ledger::ProcessResult;
use rsnano_node::{
    block_processing::{
        BlockProcessor, BlockProcessorExt, BlockProcessorImpl, BLOCKPROCESSOR_ADD_CALLBACK,
        BLOCKPROCESSOR_HALF_FULL_CALLBACK, BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK,
    },
    config::NodeConfig,
    voting::{ActiveTransactions, LocalVoteHistory},
};
use std::{
    collections::VecDeque,
    ffi::{c_char, c_void, CStr},
    ops::Deref,
    sync::{atomic::Ordering, Arc, MutexGuard},
};

pub struct BlockProcessorHandle(Arc<BlockProcessor>);

impl Deref for BlockProcessorHandle {
    type Target = Arc<BlockProcessor>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_create(
    handle: *mut c_void,
    config: &NodeConfigDto,
    checker: &SignatureCheckerHandle,
    epochs: &EpochsHandle,
    logger: *mut LoggerHandle,
    flags: &NodeFlagsHandle,
    ledger: &LedgerHandle,
    unchecked_map: &UncheckedMapHandle,
    gap_cache: &GapCacheHandle,
    stats: &StatHandle,
    work: &WorkThresholdsDto,
    write_database_queue: &WriteDatabaseQueueHandle,
    history: &LocalVoteHistoryHandle,
) -> *mut BlockProcessorHandle {
    let config = Arc::new(NodeConfig::try_from(config).unwrap());
    let checker = Arc::clone(&checker);
    let epochs = Arc::new(epochs.epochs.clone());
    let logger = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let flags = Arc::new(flags.lock().unwrap().clone());
    let ledger = Arc::clone(&ledger);
    let unchecked_map = Arc::clone(&unchecked_map);
    let gap_cache = Arc::clone(&gap_cache);
    let stats = Arc::clone(&stats);
    let work = Arc::new(WorkThresholds::from(work));
    let write_database_queue = Arc::clone(write_database_queue);
    let active = Arc::new(ActiveTransactions::new()); // TODO use real instance
    let history = Arc::clone(&history);
    let processor = Arc::new(BlockProcessor::new(
        handle,
        config,
        checker,
        epochs,
        logger,
        flags,
        ledger,
        unchecked_map,
        gap_cache,
        stats,
        work,
        write_database_queue,
        history,
        active,
    ));
    processor.init();
    Box::into_raw(Box::new(BlockProcessorHandle(processor)))
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_destroy(handle: *mut BlockProcessorHandle) {
    drop(unsafe { Box::from_raw(handle) });
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_stop(handle: &BlockProcessorHandle) {
    handle.stop().unwrap();
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_flushing(handle: &BlockProcessorHandle) -> bool {
    handle.flushing.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_is_signature_verifier_active(
    handle: &BlockProcessorHandle,
) -> bool {
    handle
        .state_block_signature_verification
        .read()
        .unwrap()
        .is_active()
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_signature_verifier_size(
    handle: &BlockProcessorHandle,
) -> usize {
    handle
        .state_block_signature_verification
        .read()
        .unwrap()
        .size()
}

pub struct BlockProcessorLockHandle(Option<MutexGuard<'static, BlockProcessorImpl>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_lock(
    handle: &BlockProcessorHandle,
) -> *mut BlockProcessorLockHandle {
    let guard = handle.mutex.lock().unwrap();
    let guard = std::mem::transmute::<
        MutexGuard<BlockProcessorImpl>,
        MutexGuard<'static, BlockProcessorImpl>,
    >(guard);
    Box::into_raw(Box::new(BlockProcessorLockHandle(Some(guard))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_lock_destroy(handle: *mut BlockProcessorLockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_lock_lock(
    handle: &mut BlockProcessorLockHandle,
    processor: &BlockProcessorHandle,
) {
    let guard = processor.0.mutex.lock().unwrap();
    let guard = std::mem::transmute::<
        MutexGuard<BlockProcessorImpl>,
        MutexGuard<'static, BlockProcessorImpl>,
    >(guard);
    handle.0 = Some(guard);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_lock_unlock(handle: &mut BlockProcessorLockHandle) {
    handle.0 = None;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_notify_all(handle: &BlockProcessorHandle) {
    handle.condition.notify_all();
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_notify_one(handle: &BlockProcessorHandle) {
    handle.condition.notify_one();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_wait(
    handle: *mut BlockProcessorHandle,
    lock_handle: *mut BlockProcessorLockHandle,
) {
    let guard = (*lock_handle).0.take().unwrap();
    let guard = (*handle).0.condition.wait(guard).unwrap();
    (*lock_handle).0 = Some(guard);
}

pub type BlockProcessorAddCallback = unsafe extern "C" fn(*mut c_void, *mut BlockHandle);
pub type BlockProcessorHalfFullCallback = unsafe extern "C" fn(*mut c_void) -> bool;
static mut ADD_CALLBACK: Option<BlockProcessorAddCallback> = None;
static mut PROCESS_ACTIVE_CALLBACK: Option<BlockProcessorAddCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_block_processor_add(f: BlockProcessorAddCallback) {
    ADD_CALLBACK = Some(f);
    BLOCKPROCESSOR_ADD_CALLBACK = Some(|handle, block| {
        ADD_CALLBACK.expect("ADD_CALLBACK missing")(
            handle,
            Box::into_raw(Box::new(BlockHandle(block))),
        )
    });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_block_processor_process_active(f: BlockProcessorAddCallback) {
    PROCESS_ACTIVE_CALLBACK = Some(f);
    BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK = Some(|handle, block| {
        PROCESS_ACTIVE_CALLBACK.expect("PROCESS_ACTIVE_CALLBACK missing")(
            handle,
            Box::into_raw(Box::new(BlockHandle(block))),
        )
    });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_block_processor_half_full(f: BlockProcessorHalfFullCallback) {
    BLOCKPROCESSOR_HALF_FULL_CALLBACK = Some(f);
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_push_back_block(
    handle: &mut BlockProcessorLockHandle,
    block: &BlockHandle,
) {
    handle
        .0
        .as_mut()
        .unwrap()
        .blocks
        .push_back(Arc::clone(&block))
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_pop_front_block(
    handle: &mut BlockProcessorLockHandle,
) -> *mut BlockHandle {
    let block = handle.0.as_mut().unwrap().blocks.pop_front();
    match block {
        Some(b) => Box::into_raw(Box::new(BlockHandle(b))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_blocks_size(handle: &mut BlockProcessorLockHandle) -> usize {
    handle.0.as_mut().unwrap().blocks.len()
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_push_back_forced(
    handle: &mut BlockProcessorLockHandle,
    block: &BlockHandle,
) {
    handle
        .0
        .as_mut()
        .unwrap()
        .forced
        .push_back(Arc::clone(&block))
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_pop_front_forced(
    handle: &mut BlockProcessorLockHandle,
) -> *mut BlockHandle {
    let block = handle.0.as_mut().unwrap().forced.pop_front();
    match block {
        Some(b) => Box::into_raw(Box::new(BlockHandle(b))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_forced_size(handle: &mut BlockProcessorLockHandle) -> usize {
    handle.0.as_mut().unwrap().forced.len()
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_should_log(handle: &mut BlockProcessorLockHandle) -> bool {
    handle.0.as_mut().unwrap().should_log()
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_add_impl(
    handle: &mut BlockProcessorHandle,
    block: &BlockHandle,
) {
    handle.add_impl(Arc::clone(&block));
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_set_flushing(handle: &mut BlockProcessorHandle, value: bool) {
    handle.flushing.store(value, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_queue_unchecked(
    handle: &BlockProcessorHandle,
    hash_or_account: *const u8,
) {
    let hash_or_account = HashOrAccount::from_ptr(hash_or_account);
    handle.queue_unchecked(&hash_or_account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_process_one(
    handle: &BlockProcessorHandle,
    txn: &mut TransactionHandle,
    block: &BlockHandle,
) -> u8 {
    let result = handle.process_one(txn.as_write_txn(), &block);
    result as u8
}

pub struct ProcessBatchResult(VecDeque<(ProcessResult, Arc<BlockEnum>)>);

#[no_mangle]
pub extern "C" fn rsn_block_processor_process_batch(
    handle: &BlockProcessorHandle,
) -> *mut ProcessBatchResult {
    let result = handle.process_batch();
    Box::into_raw(Box::new(ProcessBatchResult(result)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_process_batch_result_destroy(handle: *mut ProcessBatchResult) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_process_batch_result_size(handle: &ProcessBatchResult) -> usize {
    handle.0.len()
}

#[no_mangle]
pub extern "C" fn rsn_process_batch_result_get(
    handle: &ProcessBatchResult,
    index: usize,
    result: &mut u8,
) -> *mut BlockHandle {
    let (res, block) = &handle.0[index];
    *result = (*res) as u8;
    Box::into_raw(Box::new(BlockHandle(Arc::clone(block))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_collect_container_info(
    handle: &BlockProcessorHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = handle
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
