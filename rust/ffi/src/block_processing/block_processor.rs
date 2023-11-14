use crate::{
    core::{BlockHandle, BlockVecHandle},
    ledger::datastore::{LedgerHandle, WriteDatabaseQueueHandle},
    utils::{ContainerInfoComponentHandle, ContextWrapper, LoggerHandle, LoggerMT},
    work::WorkThresholdsDto,
    NodeConfigDto, NodeFlagsHandle, StatHandle, VoidPointerCallback,
};
use rsnano_core::{work::WorkThresholds, BlockEnum};
use rsnano_ledger::ProcessResult;
use rsnano_node::{
    block_processing::{
        BlockProcessor, BlockProcessorImpl, BLOCKPROCESSOR_ADD_CALLBACK,
        BLOCKPROCESSOR_HALF_FULL_CALLBACK, BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK,
    },
    config::NodeConfig,
};
use std::{
    collections::VecDeque,
    ffi::{c_char, c_void, CStr},
    ops::Deref,
    sync::{atomic::Ordering, Arc, MutexGuard},
};

use super::{gap_cache::GapCacheHandle, unchecked_map::UncheckedMapHandle};

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
    logger: *mut LoggerHandle,
    flags: &NodeFlagsHandle,
    ledger: &LedgerHandle,
    unchecked_map: &UncheckedMapHandle,
    gap_cache: &GapCacheHandle,
    stats: &StatHandle,
    work: &WorkThresholdsDto,
    write_database_queue: &WriteDatabaseQueueHandle,
) -> *mut BlockProcessorHandle {
    let config = Arc::new(NodeConfig::try_from(config).unwrap());
    let logger = Arc::new(LoggerMT::new(Box::from_raw(logger)));
    let flags = Arc::new(flags.lock().unwrap().clone());
    let ledger = Arc::clone(&ledger);
    let unchecked_map = Arc::clone(&unchecked_map);
    let gap_cache = Arc::clone(&gap_cache);
    let stats = Arc::clone(&stats);
    let work = Arc::new(WorkThresholds::from(work));
    let write_database_queue = Arc::clone(write_database_queue);
    let processor = Arc::new(BlockProcessor::new(
        handle,
        config,
        logger,
        flags,
        ledger,
        unchecked_map,
        gap_cache,
        stats,
        work,
        write_database_queue,
    ));
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

pub type BlocksRolledBackCallback =
    extern "C" fn(*mut c_void, *mut BlockVecHandle, *mut BlockHandle);

#[no_mangle]
pub extern "C" fn rsn_block_processor_set_blocks_rolled_back_callback(
    handle: &BlockProcessorHandle,
    callback: BlocksRolledBackCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context = ContextWrapper::new(context, delete_context);
    handle.set_blocks_rolled_back_callback(Box::new(move |rolled_back, initial_block| {
        let initial_block = BlockHandle::new(Arc::new(initial_block));
        callback(
            context.get_context(),
            BlockVecHandle::new2(rolled_back),
            initial_block,
        );
    }));
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_flushing(handle: &BlockProcessorHandle) -> bool {
    handle.flushing.load(Ordering::SeqCst)
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
