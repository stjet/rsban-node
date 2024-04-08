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
    block_processing::{BlockProcessor, BlockSource},
    config::NodeConfig,
};
use std::{
    ffi::{c_char, c_void, CStr},
    ops::Deref,
    sync::Arc,
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
pub extern "C" fn rsn_block_processor_start(handle: &BlockProcessorHandle) {
    handle.start();
}

#[no_mangle]
pub extern "C" fn rsn_block_processor_stop(handle: &BlockProcessorHandle) {
    handle.stop();
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
