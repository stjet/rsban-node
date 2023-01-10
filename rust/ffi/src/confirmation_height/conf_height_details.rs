use std::sync::{Arc, Mutex, Weak};

use rsnano_core::{Account, BlockHash};
use rsnano_node::confirmation_height::ConfHeightDetails;

use crate::copy_account_bytes;

use super::BlockHashVecHandle;

pub struct ConfHeightDetailsHandle(pub ConfHeightDetails);

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_create(
    account: *const u8,
    hash: *const u8,
    height: u64,
    num_blocks_confirmed: u64,
    block_callback_data: *const BlockHashVecHandle,
) -> *mut ConfHeightDetailsHandle {
    Box::into_raw(Box::new(ConfHeightDetailsHandle(ConfHeightDetails {
        account: Account::from_ptr(account),
        hash: BlockHash::from_ptr(hash),
        height,
        num_blocks_confirmed,
        block_callback_data: (*block_callback_data).0.clone(),
        source_block_callback_data: Vec::new(),
    })))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_clone(
    handle: *const ConfHeightDetailsHandle,
) -> *mut ConfHeightDetailsHandle {
    Box::into_raw(Box::new(ConfHeightDetailsHandle((*handle).0.clone())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_destroy(handle: *mut ConfHeightDetailsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_add_block_callback_data(
    handle: *mut ConfHeightDetailsHandle,
    data: *const u8,
) {
    (*handle)
        .0
        .block_callback_data
        .push(BlockHash::from_ptr(data));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_size() -> usize {
    std::mem::size_of::<ConfHeightDetails>()
}

// ------------------------
// Shared Pointer:

pub struct ConfHeightDetailsSharedPtrHandle(pub Arc<Mutex<ConfHeightDetails>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_ptr_create(
    details_handle: *const ConfHeightDetailsHandle,
) -> *mut ConfHeightDetailsSharedPtrHandle {
    Box::into_raw(Box::new(ConfHeightDetailsSharedPtrHandle(Arc::new(
        Mutex::new((*details_handle).0.clone()),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_ptr_clone(
    handle: *mut ConfHeightDetailsSharedPtrHandle,
) -> *mut ConfHeightDetailsSharedPtrHandle {
    Box::into_raw(Box::new(ConfHeightDetailsSharedPtrHandle(Arc::clone(
        &(*handle).0,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_ptr_destroy(
    handle: *mut ConfHeightDetailsSharedPtrHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_source_block_callback_data(
    handle: *const ConfHeightDetailsSharedPtrHandle,
) -> *mut BlockHashVecHandle {
    Box::into_raw(Box::new(BlockHashVecHandle(
        (*handle)
            .0
            .lock()
            .unwrap()
            .source_block_callback_data
            .clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_set_source_block_callback_data(
    handle: *mut ConfHeightDetailsSharedPtrHandle,
    data: *const BlockHashVecHandle,
) {
    (*handle).0.lock().unwrap().source_block_callback_data = (*data).0.clone();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_set_num_blocks_confirmed(
    handle: *mut ConfHeightDetailsSharedPtrHandle,
    confirmed: u64,
) {
    (*handle).0.lock().unwrap().num_blocks_confirmed = confirmed;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_num_blocks_confirmed(
    handle: *const ConfHeightDetailsSharedPtrHandle,
) -> u64 {
    (*handle).0.lock().unwrap().num_blocks_confirmed
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_add_block_callback_data(
    handle: *mut ConfHeightDetailsSharedPtrHandle,
    data: *const u8,
) {
    (*handle)
        .0
        .lock()
        .unwrap()
        .block_callback_data
        .push(BlockHash::from_ptr(data));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_set_block_callback_data(
    handle: *mut ConfHeightDetailsSharedPtrHandle,
    data: *const BlockHashVecHandle,
) {
    (*handle).0.lock().unwrap().block_callback_data = (*data).0.clone();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_block_callback_data(
    handle: *const ConfHeightDetailsSharedPtrHandle,
) -> *mut BlockHashVecHandle {
    Box::into_raw(Box::new(BlockHashVecHandle(
        (*handle).0.lock().unwrap().block_callback_data.clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_height(
    handle: *const ConfHeightDetailsSharedPtrHandle,
) -> u64 {
    (*handle).0.lock().unwrap().height
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_account(
    handle: *const ConfHeightDetailsSharedPtrHandle,
    account: *mut u8,
) {
    copy_account_bytes((*handle).0.lock().unwrap().account, account);
}

// ------------------------
// Weak Pointer:

pub struct ConfHeightDetailsWeakPtrHandle(pub Weak<Mutex<ConfHeightDetails>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_shared_ptr_to_weak(
    handle: *mut ConfHeightDetailsSharedPtrHandle,
) -> *mut ConfHeightDetailsWeakPtrHandle {
    Box::into_raw(Box::new(ConfHeightDetailsWeakPtrHandle(Arc::downgrade(
        &(*handle).0,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_weak_ptr_clone(
    handle: *mut ConfHeightDetailsWeakPtrHandle,
) -> *mut ConfHeightDetailsWeakPtrHandle {
    Box::into_raw(Box::new(ConfHeightDetailsWeakPtrHandle(Weak::clone(
        &(*handle).0,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_weak_ptr_destroy(
    handle: *mut ConfHeightDetailsWeakPtrHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_weak_expired(
    handle: *mut ConfHeightDetailsWeakPtrHandle,
) -> bool {
    (*handle).0.strong_count() == 0
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_weak_upgrade(
    handle: *mut ConfHeightDetailsWeakPtrHandle,
) -> *mut ConfHeightDetailsSharedPtrHandle {
    let details = (*handle).0.upgrade().unwrap();
    Box::into_raw(Box::new(ConfHeightDetailsSharedPtrHandle(details)))
}
