use rsnano_core::{Account, BlockHash};
use rsnano_node::confirmation_height::ConfHeightDetails;

use crate::{copy_account_bytes, copy_hash_bytes, U256ArrayDto};

pub struct ConfHeightDetailsHandle(ConfHeightDetails);

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_create(
    account: *const u8,
    hash: *const u8,
    height: u64,
    num_blocks_confirmed: u64,
) -> *mut ConfHeightDetailsHandle {
    Box::into_raw(Box::new(ConfHeightDetailsHandle(ConfHeightDetails {
        account: Account::from_ptr(account),
        hash: BlockHash::from_ptr(hash),
        height,
        num_blocks_confirmed,
        block_callback_data: Vec::new(),
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
pub unsafe extern "C" fn rsn_conf_height_details_account(
    handle: *const ConfHeightDetailsHandle,
    account: *mut u8,
) {
    copy_account_bytes((*handle).0.account, account);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_hash(
    handle: *const ConfHeightDetailsHandle,
    hash: *mut u8,
) {
    copy_hash_bytes((*handle).0.hash, hash);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_height(
    handle: *const ConfHeightDetailsHandle,
) -> u64 {
    (*handle).0.height
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_num_blocks_confirmed(
    handle: *const ConfHeightDetailsHandle,
) -> u64 {
    (*handle).0.num_blocks_confirmed
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_set_num_blocks_confirmed(
    handle: *mut ConfHeightDetailsHandle,
    confirmed: u64,
) {
    (*handle).0.num_blocks_confirmed = confirmed;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_block_callback_data(
    handle: *const ConfHeightDetailsHandle,
    result: *mut U256ArrayDto,
) {
    let data = Box::new(
        (*handle)
            .0
            .block_callback_data
            .iter()
            .map(|a| *a.as_bytes())
            .collect(),
    );
    (*result).initialize(data);
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
pub unsafe extern "C" fn rsn_conf_height_details_set_block_callback_data(
    handle: *mut ConfHeightDetailsHandle,
    data: *const *const u8,
    len: usize,
) {
    (*handle).0.block_callback_data = std::slice::from_raw_parts(data, len)
        .iter()
        .map(|&bytes| BlockHash::from_ptr(bytes))
        .collect();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_source_block_callback_data(
    handle: *const ConfHeightDetailsHandle,
    result: *mut U256ArrayDto,
) {
    let data = Box::new(
        (*handle)
            .0
            .source_block_callback_data
            .iter()
            .map(|a| *a.as_bytes())
            .collect(),
    );
    (*result).initialize(data);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_conf_height_details_set_source_block_callback_data(
    handle: *mut ConfHeightDetailsHandle,
    data: *const *const u8,
    len: usize,
) {
    (*handle).0.source_block_callback_data = std::slice::from_raw_parts(data, len)
        .iter()
        .map(|&bytes| BlockHash::from_ptr(bytes))
        .collect();
}
