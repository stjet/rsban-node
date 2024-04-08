use super::unchecked_map::UncheckedMapHandle;
use crate::{
    core::{BlockHandle, BlockVecHandle},
    ledger::datastore::LedgerHandle,
    transport::ChannelHandle,
    utils::{ContainerInfoComponentHandle, ContextWrapper},
    work::WorkThresholdsDto,
    NodeConfigDto, NodeFlagsHandle, StatHandle, VoidPointerCallback,
};
use num_traits::FromPrimitive;
use rsnano_core::work::WorkThresholds;
use rsnano_node::{
    block_processing::{
        BlockProcessor, BlockProcessorContext, BlockProcessorImpl, BlockSource,
        BLOCKPROCESSOR_PROCESS_ACTIVE_CALLBACK,
    },
    config::NodeConfig,
};
use std::{
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
) -> *mut BlockProcessorHandle {
    let config = Arc::new(NodeConfig::try_from(config).unwrap());
    let flags = Arc::new(flags.lock().unwrap().clone());
    let ledger = Arc::clone(&ledger);
    let unchecked_map = Arc::clone(&unchecked_map);
    let stats = Arc::clone(&stats);
    let work = Arc::new(WorkThresholds::from(work));
    let processor = Arc::new(BlockProcessor::new(
        handle,
        config,
        flags,
        ledger,
        unchecked_map,
        stats,
        work,
    ));
    Box::into_raw(Box::new(BlockProcessorHandle(processor)))
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_destroy(handle: *mut BlockProcessorHandle) {
    drop(unsafe { Box::from_raw(handle) });
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_run(handle: &BlockProcessorHandle) {
    handle.run();
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
pub extern "C" fn rsn_block_processor_add_blocking(
    handle: &mut BlockProcessorHandle,
    block: &BlockHandle,
    source: u8,
    status: &mut u8,
) -> bool {
    match handle.add_blocking(Arc::clone(block), BlockSource::from_u8(source).unwrap()) {
        Some(i) => {
            *status = i as u8;
            true
        }
        None => false,
    }
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_force(
    handle: &mut BlockProcessorHandle,
    block: &BlockHandle,
) {
    handle.force(Arc::clone(block));
}

#[repr(C)]
pub struct BlockProcessedInfoDto {
    pub status: u8,
    pub block: *mut BlockHandle,
    pub source: u8,
}

pub type BlockProcessedCallback = extern "C" fn(*mut c_void, *mut BlockProcessedInfoDto);

#[no_mangle]
pub extern "C" fn rsn_block_processor_add_block_processed_observer(
    handle: &mut BlockProcessorHandle,
    context: *mut c_void,
    drop_context: VoidPointerCallback,
    observer: BlockProcessedCallback,
) {
    let context_wrapper = ContextWrapper::new(context, drop_context);
    handle.add_block_processed_observer(Box::new(move |status, block_context| {
        let block_handle = BlockHandle::new(Arc::clone(&block_context.block));
        let mut dto = BlockProcessedInfoDto {
            status: status as u8,
            block: block_handle,
            source: block_context.source as u8,
        };
        observer(context_wrapper.get_context(), &mut dto);
    }));
}

pub type BatchProcessedCallback = extern "C" fn(*mut c_void, *const BlockProcessedInfoDto, usize);

#[no_mangle]
pub extern "C" fn rsn_block_processor_add_batch_processed_observer(
    handle: &mut BlockProcessorHandle,
    context: *mut c_void,
    drop_context: VoidPointerCallback,
    observer: BatchProcessedCallback,
) {
    let context_wrapper = ContextWrapper::new(context, drop_context);
    handle.add_batch_processed_observer(Box::new(move |blocks| {
        let dtos = blocks
            .iter()
            .map(|(status, context)| BlockProcessedInfoDto {
                status: *status as u8,
                block: BlockHandle::new(Arc::clone(&context.block)),
                source: context.source as u8,
            })
            .collect::<Vec<_>>();

        observer(context_wrapper.get_context(), dtos.as_ptr(), dtos.len());
    }));
}

pub type BlockRolledBackCallback = extern "C" fn(*mut c_void, *mut BlockHandle);

#[no_mangle]
pub extern "C" fn rsn_block_processor_add_rolled_back_observer(
    handle: &mut BlockProcessorHandle,
    context: *mut c_void,
    drop_context: VoidPointerCallback,
    observer: BlockRolledBackCallback,
) {
    let context_wrapper = ContextWrapper::new(context, drop_context);
    handle.add_rolled_back_observer(Box::new(move |block| {
        let block_handle = BlockHandle::new(Arc::new(block.clone()));
        observer(context_wrapper.get_context(), block_handle);
    }));
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_notify_block_rolled_back(
    handle: &mut BlockProcessorHandle,
    block: &BlockHandle,
) {
    handle.notify_block_rolled_back(block);
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

impl BlockProcessorContextHandle {
    pub fn new(ctx: BlockProcessorContext) -> *mut Self {
        Box::into_raw(Box::new(Self(Some(ctx))))
    }
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_context_create(
    block: &BlockHandle,
    source: u8,
) -> *mut BlockProcessorContextHandle {
    BlockProcessorContextHandle::new(BlockProcessorContext::new(
        Arc::clone(block),
        FromPrimitive::from_u8(source).unwrap(),
    ))
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

pub type PromiseSetResultCallback = unsafe extern "C" fn(*mut c_void, u8);

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_block_processor_promise_set_result(
    callback: PromiseSetResultCallback,
) {
    BLOCK_PROCESSOR_PROMISE_SET_RESULT = Some(callback);
}
