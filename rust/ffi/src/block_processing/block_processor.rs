use super::unchecked_map::UncheckedMapHandle;
use crate::{
    core::{BlockHandle, BlockVecHandle},
    ledger::datastore::{LedgerHandle, WriteQueueHandle},
    transport::ChannelHandle,
    utils::{ContainerInfoComponentHandle, ContextWrapper},
    work::WorkThresholdsDto,
    NodeConfigDto, NodeFlagsHandle, StatHandle, VoidPointerCallback,
};
use num_traits::FromPrimitive;
use rsnano_core::work::WorkThresholds;
use rsnano_ledger::BlockStatus;
use rsnano_node::{
    block_processing::{
        BlockProcessor, BlockProcessorContext, BlockProcessorImpl, BlockSource,
        BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK, CREATE_BLOCK_PROCESSOR_PROMISE,
        DROP_BLOCK_PROCESSOR_PROMISE,
    },
    config::NodeConfig,
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
    flags: &NodeFlagsHandle,
    ledger: &LedgerHandle,
    unchecked_map: &UncheckedMapHandle,
    stats: &StatHandle,
    work: &WorkThresholdsDto,
    write_queue: &WriteQueueHandle,
) -> *mut BlockProcessorHandle {
    let config = Arc::new(NodeConfig::try_from(config).unwrap());
    let flags = Arc::new(flags.lock().unwrap().clone());
    let ledger = Arc::clone(&ledger);
    let unchecked_map = Arc::clone(&unchecked_map);
    let stats = Arc::clone(&stats);
    let work = Arc::new(WorkThresholds::from(work));
    let write_queue = Arc::clone(write_queue);
    let processor = Arc::new(BlockProcessor::new(
        handle,
        config,
        flags,
        ledger,
        unchecked_map,
        stats,
        work,
        write_queue,
    ));
    Box::into_raw(Box::new(BlockProcessorHandle(processor)))
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_destroy(handle: *mut BlockProcessorHandle) {
    drop(unsafe { Box::from_raw(handle) });
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_total_queue_len(handle: &BlockProcessorHandle) -> usize {
    handle.total_queue_len()
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_queue_len(
    handle: &BlockProcessorHandle,
    source: u8,
) -> usize {
    handle.queue_len(BlockSource::from_u8(source).unwrap())
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_full(handle: &BlockProcessorHandle) -> bool {
    handle.full()
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_half_full(handle: &BlockProcessorHandle) -> bool {
    handle.half_full()
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
pub unsafe extern "C" fn rsn_block_processor_lock_queue_empty(
    handle: &BlockProcessorLockHandle,
) -> bool {
    handle.0.as_ref().unwrap().queue.is_empty()
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

pub type BlockProcessorAddCallback = unsafe extern "C" fn(*mut c_void, *mut BlockHandle, u8);
pub type BlockProcessorProcessActiveCallback = unsafe extern "C" fn(*mut c_void, *mut BlockHandle);
pub type BlockProcessorHalfFullCallback = unsafe extern "C" fn(*mut c_void) -> bool;
pub type BlockProcessorSizeCallback = unsafe extern "C" fn(*mut c_void) -> usize;

static mut ADD_CALLBACK: Option<BlockProcessorAddCallback> = None;
static mut PROCESS_ACTIVE_CALLBACK: Option<BlockProcessorProcessActiveCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_block_processor_process_active(
    f: BlockProcessorProcessActiveCallback,
) {
    PROCESS_ACTIVE_CALLBACK = Some(f);
    BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK = Some(|handle, block| {
        PROCESS_ACTIVE_CALLBACK.expect("PROCESS_ACTIVE_CALLBACK missing")(
            handle,
            Box::into_raw(Box::new(BlockHandle(block))),
        )
    });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_add(
    handle: &mut BlockProcessorHandle,
    block: &BlockHandle,
    source: u8,
    channel: *const ChannelHandle,
) -> bool {
    let channel = if channel.is_null() {
        None
    } else {
        Some(Arc::clone(&*channel))
    };
    handle.add(
        Arc::clone(block),
        FromPrimitive::from_u8(source).unwrap(),
        channel,
    )
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_add_impl(
    handle: &mut BlockProcessorHandle,
    context: &mut BlockProcessorContextHandle,
    channel: *mut ChannelHandle,
) -> bool {
    let channel = if channel.is_null() {
        None
    } else {
        Some(Arc::clone(&*channel))
    };
    handle.add_impl(context.0.take().unwrap(), channel)
}

pub struct ProcessBatchResult(VecDeque<(BlockStatus, BlockProcessorContext)>);

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
pub extern "C" fn rsn_process_batch_result_pop(
    handle: &mut ProcessBatchResult,
    result: &mut u8,
) -> *mut BlockProcessorContextHandle {
    let (res, ctx) = handle.0.pop_front().unwrap();
    *result = res as u8;
    Box::into_raw(Box::new(BlockProcessorContextHandle(Some(ctx))))
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

pub struct BlockProcessorContextHandle(Option<BlockProcessorContext>);

#[no_mangle]
pub extern "C" fn rsn_block_processor_context_create(
    block: &BlockHandle,
    source: u8,
) -> *mut BlockProcessorContextHandle {
    Box::into_raw(Box::new(BlockProcessorContextHandle(Some(
        BlockProcessorContext::new(Arc::clone(block), FromPrimitive::from_u8(source).unwrap()),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_context_destroy(
    handle: *mut BlockProcessorContextHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_context_block(
    handle: &BlockProcessorContextHandle,
) -> *mut BlockHandle {
    BlockHandle::new(Arc::clone(&handle.0.as_ref().unwrap().block))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_context_source(
    handle: &BlockProcessorContextHandle,
) -> u8 {
    handle.0.as_ref().unwrap().source as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_processor_context_promise(
    handle: &BlockProcessorContextHandle,
) -> *mut c_void {
    handle.0.as_ref().unwrap().promise
}

pub type VoidPointerResultCallback = unsafe extern "C" fn() -> *mut c_void;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_create_block_processor_promise(
    callback: VoidPointerResultCallback,
) {
    CREATE_BLOCK_PROCESSOR_PROMISE = Some(callback);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_drop_block_processor_promise(callback: VoidPointerCallback) {
    DROP_BLOCK_PROCESSOR_PROMISE = Some(callback);
}
