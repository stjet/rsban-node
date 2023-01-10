mod conf_height_details;

use std::{
    ffi::c_void,
    ops::Deref,
    sync::{atomic::Ordering, Arc, Mutex, RwLock, Weak},
    time::Duration,
};

use rsnano_core::{Account, BlockEnum, BlockHash};
use rsnano_node::{
    config::Logging,
    confirmation_height::{
        ConfHeightDetails, ConfirmationHeightUnbounded, ConfirmedIteratedPair, ReceiveSourcePair,
    },
};

use crate::{
    copy_hash_bytes,
    core::BlockHandle,
    ledger::datastore::{LedgerHandle, TransactionHandle, WriteGuardHandle},
    utils::{LoggerHandle, LoggerMT},
    LoggingDto, StatHandle, VoidPointerCallback,
};

use self::conf_height_details::{
    ConfHeightDetailsHandle, ConfHeightDetailsSharedPtrHandle, ConfHeightDetailsWeakPtrHandle,
};

pub struct ConfirmationHeightUnboundedHandle(ConfirmationHeightUnbounded);

pub type ConfHeightUnboundedNotifyObserversCallback =
    unsafe extern "C" fn(*mut c_void, *const *mut BlockHandle, usize);

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_create(
    ledger: *const LedgerHandle,
    logger: *mut LoggerHandle,
    logging: *const LoggingDto,
    stats: *const StatHandle,
    batch_separate_pending_min_time_ms: u64,
    notify_observers: ConfHeightUnboundedNotifyObserversCallback,
    notify_observers_context: *mut c_void,
    drop_notify_observers_context: VoidPointerCallback,
) -> *mut ConfirmationHeightUnboundedHandle {
    let notify_observers_callback = wrap_notify_observers_callback(
        notify_observers,
        notify_observers_context,
        drop_notify_observers_context,
    );

    let result = Box::into_raw(Box::new(ConfirmationHeightUnboundedHandle(
        ConfirmationHeightUnbounded::new(
            Arc::clone(&(*ledger).0),
            Arc::new(LoggerMT::new(Box::from_raw(logger))),
            Logging::from(&*logging),
            Arc::clone(&(*stats).0),
            Duration::from_millis(batch_separate_pending_min_time_ms),
            notify_observers_callback,
        ),
    )));
    result
}

struct NotifyObserversCallbackContextWrapper {
    context: *mut c_void,
    drop_context: VoidPointerCallback,
}

impl NotifyObserversCallbackContextWrapper {
    fn new(context: *mut c_void, drop_context: VoidPointerCallback) -> Self {
        Self {
            context,
            drop_context,
        }
    }

    fn get_context(&self) -> *mut c_void {
        self.context
    }
}

impl Drop for NotifyObserversCallbackContextWrapper {
    fn drop(&mut self) {
        unsafe {
            (self.drop_context)(self.context);
        }
    }
}

unsafe fn wrap_notify_observers_callback(
    callback: ConfHeightUnboundedNotifyObserversCallback,
    context: *mut c_void,
    drop_context: VoidPointerCallback,
) -> Box<dyn Fn(&Vec<Arc<BlockEnum>>)> {
    let context_wrapper = NotifyObserversCallbackContextWrapper::new(context, drop_context);

    Box::new(move |blocks| {
        let block_handles = blocks
            .iter()
            .map(|b| {
                Box::into_raw(Box::new(BlockHandle::new(Arc::new(RwLock::new(
                    b.deref().clone(),
                )))))
            })
            .collect::<Vec<_>>();

        callback(
            context_wrapper.get_context(),
            block_handles.as_ptr(),
            block_handles.len(),
        );

        for handle in block_handles {
            drop(Box::from_raw(handle))
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_destroy(
    handle: *mut ConfirmationHeightUnboundedHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_add(
    handle: *mut ConfirmationHeightUnboundedHandle,
    details: *const ConfHeightDetailsHandle,
) {
    (*handle).0.add_pending_write((*details).0.clone());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_add2(
    handle: *mut ConfirmationHeightUnboundedHandle,
    details: *const ConfHeightDetailsSharedPtrHandle,
) {
    (*handle)
        .0
        .add_pending_write((*details).0.lock().unwrap().clone());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_size(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> usize {
    (*handle).0.pending_writes.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_total_pending_write_block_count(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> u64 {
    (*handle).0.total_pending_write_block_count()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_empty(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> bool {
    (*handle).0.pending_empty()
}

#[repr(C)]
pub struct ConfirmedIteratedPairsIteratorDto {
    pub is_end: bool,
    pub account: [u8; 32],
    pub confirmed_height: u64,
    pub iterated_height: u64,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_conf_iterated_pairs_insert(
    handle: *mut ConfirmationHeightUnboundedHandle,
    account: *const u8,
    confirmed_height: u64,
    iterated_height: u64,
) {
    (*handle).0.add_confirmed_iterated_pair(
        Account::from_ptr(account),
        confirmed_height,
        iterated_height,
    );
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_conf_iterated_pairs_len(
    handle: *const ConfirmationHeightUnboundedHandle,
) -> usize {
    (*handle)
        .0
        .confirmed_iterated_pairs_size
        .load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_len(
    handle: *const ConfirmationHeightUnboundedHandle,
) -> usize {
    (*handle).0.pending_writes_size.load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_iterated_pair_size() -> usize {
    std::mem::size_of::<ConfirmedIteratedPair>()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_conf_iterated_pairs_find(
    handle: *mut ConfirmationHeightUnboundedHandle,
    account: *const u8,
    result: *mut ConfirmedIteratedPairsIteratorDto,
) {
    let account = Account::from_ptr(account);
    let res = &mut *result;
    match (*handle).0.confirmed_iterated_pairs.get(&account) {
        Some(pair) => {
            res.is_end = false;
            res.account = *account.as_bytes();
            res.confirmed_height = pair.confirmed_height;
            res.iterated_height = pair.iterated_height;
        }
        None => res.is_end = true,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_conf_iterated_pairs_set_confirmed_height(
    handle: *mut ConfirmationHeightUnboundedHandle,
    account: *const u8,
    height: u64,
) {
    let account = Account::from_ptr(account);
    let pair = (*handle)
        .0
        .confirmed_iterated_pairs
        .get_mut(&account)
        .unwrap();
    pair.confirmed_height = height;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_conf_iterated_pairs_set_iterated_height(
    handle: *mut ConfirmationHeightUnboundedHandle,
    account: *const u8,
    height: u64,
) {
    let account = Account::from_ptr(account);
    let pair = (*handle)
        .0
        .confirmed_iterated_pairs
        .get_mut(&account)
        .unwrap();
    pair.iterated_height = height;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_implicit_receive_cemented_mapping_add(
    handle: *mut ConfirmationHeightUnboundedHandle,
    hash: *const u8,
    details: *const ConfHeightDetailsSharedPtrHandle,
) {
    let hash = BlockHash::from_ptr(hash);
    (*handle)
        .0
        .add_implicit_receive_cemented(hash, &(*details).0);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_get_implicit_receive_cemented(
    handle: *mut ConfirmationHeightUnboundedHandle,
    hash: *const u8,
) -> *mut ConfHeightDetailsWeakPtrHandle {
    let hash = BlockHash::from_ptr(hash);
    let weak = (*handle).0.get_implicit_receive_cemented(&hash).unwrap();
    Box::into_raw(Box::new(ConfHeightDetailsWeakPtrHandle(Weak::clone(weak))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_implicit_receive_cemented_mapping_size(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> usize {
    (*handle)
        .0
        .implicit_receive_cemented_mapping_size
        .load(Ordering::Relaxed)
}

#[no_mangle]
pub extern "C" fn rsn_implicit_receive_cemented_mapping_value_size() -> usize {
    std::mem::size_of::<Weak<Mutex<ConfHeightDetails>>>()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_cache_block(
    handle: *mut ConfirmationHeightUnboundedHandle,
    block: *const BlockHandle,
) {
    (*handle)
        .0
        .cache_block(Arc::new((*block).block.read().unwrap().clone()))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_get_block_and_sideband(
    handle: *mut ConfirmationHeightUnboundedHandle,
    hash: *const u8,
    txn: *const TransactionHandle,
) -> *mut BlockHandle {
    let block = (*handle)
        .0
        .get_block_and_sideband(&BlockHash::from_ptr(hash), (*txn).as_txn());
    Box::into_raw(Box::new(BlockHandle::new(Arc::new(RwLock::new(
        block.deref().clone(),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_has_iterated_over_block(
    handle: *const ConfirmationHeightUnboundedHandle,
    hash: *const u8,
) -> bool {
    (*handle)
        .0
        .has_iterated_over_block(&BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_block_cache_size(
    handle: *const ConfirmationHeightUnboundedHandle,
) -> usize {
    (*handle).0.block_cache_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_block_cache_element_size() -> usize {
    std::mem::size_of::<Arc<BlockEnum>>()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_restart_timer(
    handle: *mut ConfirmationHeightUnboundedHandle,
) {
    (*handle).0.restart_timer();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_size_safe(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> usize {
    (*handle).0.pending_writes_size.load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_min_time_exceeded(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> bool {
    (*handle).0.min_time_exceeded()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_clear_process_vars(
    handle: *mut ConfirmationHeightUnboundedHandle,
) {
    (*handle).0.clear_process_vars()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_cement_blocks(
    handle: *mut ConfirmationHeightUnboundedHandle,
    write_guard: *mut WriteGuardHandle,
) {
    (*handle).0.cement_blocks(&mut (*write_guard).0);
}

pub struct ReceiveSourcePairHandle(Arc<ReceiveSourcePair>);

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_create(
    details: *mut ConfHeightDetailsSharedPtrHandle,
    hash: *const u8,
) -> *mut ReceiveSourcePairHandle {
    let details = Arc::clone(&(*details).0);
    let hash = BlockHash::from_ptr(hash);
    Box::into_raw(Box::new(ReceiveSourcePairHandle(Arc::new(
        ReceiveSourcePair::new(details, hash),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_clone(
    other: *const ReceiveSourcePairHandle,
) -> *mut ReceiveSourcePairHandle {
    Box::into_raw(Box::new(ReceiveSourcePairHandle((*other).0.clone())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_destroy(handle: *mut ReceiveSourcePairHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_receive_details(
    handle: *const ReceiveSourcePairHandle,
) -> *mut ConfHeightDetailsSharedPtrHandle {
    Box::into_raw(Box::new(ConfHeightDetailsSharedPtrHandle(Arc::clone(
        &(*handle).0.receive_details,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_source_hash(
    handle: *const ReceiveSourcePairHandle,
    result: *mut u8,
) {
    copy_hash_bytes((*handle).0.source_hash, result)
}

pub struct ReceiveSourcePairVecHandle(Vec<Arc<ReceiveSourcePair>>);

#[no_mangle]
pub extern "C" fn rsn_receive_source_pair_vec_create() -> *mut ReceiveSourcePairVecHandle {
    Box::into_raw(Box::new(ReceiveSourcePairVecHandle(Vec::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_vec_destroy(
    handle: *mut ReceiveSourcePairVecHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_vec_size(
    handle: *mut ReceiveSourcePairVecHandle,
) -> usize {
    (*handle).0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_vec_push(
    handle: *mut ReceiveSourcePairVecHandle,
    pair: *const ReceiveSourcePairHandle,
) {
    (*handle).0.push(Arc::clone(&(*pair).0));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_vec_pop(handle: *mut ReceiveSourcePairVecHandle) {
    (*handle).0.pop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_source_pair_vec_back(
    handle: *mut ReceiveSourcePairVecHandle,
) -> *mut ReceiveSourcePairHandle {
    Box::into_raw(Box::new(ReceiveSourcePairHandle(Arc::clone(
        (*handle).0.last().unwrap(),
    ))))
}
