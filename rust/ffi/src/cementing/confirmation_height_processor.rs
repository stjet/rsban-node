use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use rsnano_core::BlockHash;
use rsnano_node::cementing::{ConfirmationHeightProcessor, GuardedData};

use crate::{core::BlockHandle, ledger::datastore::WriteDatabaseQueueHandle};

pub struct ConfirmationHeightProcessorHandle(ConfirmationHeightProcessor);

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_create(
    write_database_queue: *mut WriteDatabaseQueueHandle,
) -> *mut ConfirmationHeightProcessorHandle {
    Box::into_raw(Box::new(ConfirmationHeightProcessorHandle(
        ConfirmationHeightProcessor::new((*write_database_queue).0.clone()),
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
    (*handle).0.add((*block).block.clone());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_set_next_hash(
    handle: *mut ConfirmationHeightProcessorHandle,
) {
    (*handle).0.set_next_hash();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_awaiting_processing_entry_size() -> usize
{
    ConfirmationHeightProcessor::awaiting_processing_entry_size()
}

//----------------------------------------
// Mutex
//----------------------------------------

pub struct ConfirmationHeightProcessorMutex(Arc<Mutex<GuardedData>>);
pub struct ConfirmationHeightProcessorLock {
    mutex: Arc<Mutex<GuardedData>>,
    guard: Option<MutexGuard<'static, GuardedData>>,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_get_mutex(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> *mut ConfirmationHeightProcessorMutex {
    Box::into_raw(Box::new(ConfirmationHeightProcessorMutex(
        (*handle).0.guarded_data.clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_mutex_destroy(
    handle: *mut ConfirmationHeightProcessorMutex,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_mutex_lock(
    handle: *mut ConfirmationHeightProcessorMutex,
) -> *mut ConfirmationHeightProcessorLock {
    let guard = (*handle).0.lock().unwrap();
    let guard =
        std::mem::transmute::<MutexGuard<GuardedData>, MutexGuard<'static, GuardedData>>(guard);
    Box::into_raw(Box::new(ConfirmationHeightProcessorLock {
        mutex: (*handle).0.clone(),
        guard: Some(guard),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_lock_destroy(
    handle: *mut ConfirmationHeightProcessorLock,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_lock_unlock(
    handle: *mut ConfirmationHeightProcessorLock,
) {
    drop((*handle).guard.take());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_lock_relock(
    handle: *mut ConfirmationHeightProcessorLock,
) {
    drop((*handle).guard.take());
    let guard = (*handle).mutex.lock().unwrap();
    let guard =
        std::mem::transmute::<MutexGuard<GuardedData>, MutexGuard<'static, GuardedData>>(guard);
    (*handle).guard = Some(guard);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_lock_paused(
    handle: *mut ConfirmationHeightProcessorLock,
) -> bool {
    (*handle).guard.as_ref().unwrap().paused
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_lock_paused_set(
    handle: *mut ConfirmationHeightProcessorLock,
    value: bool,
) {
    (*handle).guard.as_mut().unwrap().paused = value;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_awaiting_processing_push_back(
    handle: *mut ConfirmationHeightProcessorLock,
    block: *const BlockHandle,
) {
    (*handle)
        .guard
        .as_mut()
        .unwrap()
        .awaiting_processing
        .push_back((*block).block.clone());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_awaiting_processing_front(
    handle: *mut ConfirmationHeightProcessorLock,
) -> *mut BlockHandle {
    let front = (*handle)
        .guard
        .as_ref()
        .unwrap()
        .awaiting_processing
        .front();

    match front {
        Some(block) => Box::into_raw(Box::new(BlockHandle::new(block.clone()))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_awaiting_processing_pop_front(
    handle: *mut ConfirmationHeightProcessorLock,
) {
    (*handle)
        .guard
        .as_mut()
        .unwrap()
        .awaiting_processing
        .pop_front();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_awaiting_processing_empty(
    handle: *mut ConfirmationHeightProcessorLock,
) -> bool {
    (*handle)
        .guard
        .as_ref()
        .unwrap()
        .awaiting_processing
        .is_empty()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_awaiting_processing_size(
    handle: *mut ConfirmationHeightProcessorLock,
) -> usize {
    (*handle).guard.as_ref().unwrap().awaiting_processing.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_awaiting_processing_contains(
    handle: *mut ConfirmationHeightProcessorLock,
    hash: *const u8,
) -> bool {
    (*handle)
        .guard
        .as_ref()
        .unwrap()
        .awaiting_processing
        .contains(&BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_original_hashes_pending_contains(
    handle: *mut ConfirmationHeightProcessorLock,
    hash: *const u8,
) -> bool {
    (*handle)
        .guard
        .as_ref()
        .unwrap()
        .original_hashes_pending
        .contains(&BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_original_hashes_pending_insert(
    handle: *mut ConfirmationHeightProcessorLock,
    hash: *const u8,
) {
    (*handle)
        .guard
        .as_mut()
        .unwrap()
        .original_hashes_pending
        .insert(BlockHash::from_ptr(hash));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_original_hashes_pending_clear(
    handle: *mut ConfirmationHeightProcessorLock,
) {
    (*handle)
        .guard
        .as_mut()
        .unwrap()
        .original_hashes_pending
        .clear()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_original_block(
    handle: *mut ConfirmationHeightProcessorLock,
) -> *mut BlockHandle {
    let block = &(*handle).guard.as_ref().unwrap().original_block;
    match block {
        Some(block) => Box::into_raw(Box::new(BlockHandle::new(block.clone()))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_original_block_set(
    handle: *mut ConfirmationHeightProcessorLock,
    block: *const BlockHandle,
) {
    let new_block = if block.is_null() {
        None
    } else {
        Some((*block).block.clone())
    };

    (*handle).guard.as_mut().unwrap().original_block = new_block;
}

//----------------------------------------
// Condvar
//----------------------------------------

pub struct ConfirmationHeightProcessorCondvar(Arc<Condvar>);

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_get_condvar(
    handle: *mut ConfirmationHeightProcessorHandle,
) -> *mut ConfirmationHeightProcessorCondvar {
    Box::into_raw(Box::new(ConfirmationHeightProcessorCondvar(
        (*handle).0.condition.clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_condvar_destroy(
    handle: *mut ConfirmationHeightProcessorCondvar,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_condvar_notify_one(
    handle: *mut ConfirmationHeightProcessorCondvar,
) {
    (*handle).0.notify_one();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_height_processor_condvar_wait(
    handle: *mut ConfirmationHeightProcessorCondvar,
    lock: *mut ConfirmationHeightProcessorLock,
) {
    let guard = (*lock).guard.take().unwrap();
    let guard = (*handle).0.wait(guard).unwrap();
    (*lock).guard = Some(guard);
}
