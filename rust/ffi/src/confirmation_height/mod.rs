mod conf_height_details;

use std::{
    ops::Deref,
    sync::{atomic::Ordering, Arc, Mutex, RwLock, Weak},
    time::Duration,
};

use rsnano_core::{Account, BlockEnum, BlockHash};
use rsnano_node::confirmation_height::{
    ConfHeightDetails, ConfirmationHeightUnbounded, ConfirmedIteratedPair,
};

use crate::{
    core::{copy_block_array_dto, BlockArrayDto, BlockHandle},
    ledger::datastore::{LedgerHandle, TransactionHandle},
};

use self::conf_height_details::{
    ConfHeightDetailsHandle, ConfHeightDetailsSharedPtrHandle, ConfHeightDetailsWeakPtrHandle,
};

pub struct ConfirmationHeightUnboundedHandle(ConfirmationHeightUnbounded);

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_create(
    ledger: *const LedgerHandle,
    batch_separate_pending_min_time_ms: u64,
) -> *mut ConfirmationHeightUnboundedHandle {
    Box::into_raw(Box::new(ConfirmationHeightUnboundedHandle(
        ConfirmationHeightUnbounded::new(
            Arc::clone(&(*ledger).0),
            Duration::from_millis(batch_separate_pending_min_time_ms),
        ),
    )))
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
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_erase_first(
    handle: *mut ConfirmationHeightUnboundedHandle,
) {
    (*handle).0.erase_first_pending_write();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_size(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> usize {
    (*handle).0.pending_writes.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_unbounded_pending_writes_front(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> *mut ConfHeightDetailsHandle {
    Box::into_raw(Box::new(ConfHeightDetailsHandle(
        (*handle).0.pending_writes.front().unwrap().clone(),
    )))
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
pub unsafe extern "C" fn rsn_conf_height_unbounded_conf_iterated_pairs_clear(
    handle: *mut ConfirmationHeightUnboundedHandle,
) {
    (*handle).0.clear_confirmed_iterated_pairs();
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
pub unsafe extern "C" fn rsn_conf_height_unbounded_implicit_receive_cemented_mapping_clear(
    handle: *mut ConfirmationHeightUnboundedHandle,
) {
    (*handle).0.clear_implicit_receive_cemented_mapping();
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
pub unsafe extern "C" fn rsn_conf_height_unbounded_get_blocks(
    handle: *mut ConfirmationHeightUnboundedHandle,
    details: *const ConfHeightDetailsHandle,
    result: *mut BlockArrayDto,
) {
    let blocks = (*handle).0.get_blocks(&(*details).0);
    let blocks = blocks
        .iter()
        .map(|b| Arc::new(RwLock::new(b.deref().clone())))
        .collect();
    copy_block_array_dto(blocks, result);
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
pub unsafe extern "C" fn rsn_conf_height_unbounded_clear_block_cache(
    handle: *mut ConfirmationHeightUnboundedHandle,
) {
    (*handle).0.clear_block_cache();
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
pub unsafe extern "C" fn rsn_conf_height_unbounded_min_time_exceeded(
    handle: *mut ConfirmationHeightUnboundedHandle,
) -> bool {
    (*handle).0.min_time_exceeded()
}
