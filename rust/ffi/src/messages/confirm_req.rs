use super::{create_message_handle2, message_handle_clone, MessageHandle};
use crate::{core::BlockHandle, NetworkConstantsDto, StringDto};
use rsnano_core::{BlockHash, Root};
use rsnano_node::messages::{ConfirmReq, Message};
use std::{ops::Deref, sync::Arc};

#[repr(C)]
pub struct HashRootPair {
    pub block_hash: [u8; 32],
    pub root: [u8; 32],
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_create(
    constants: *mut NetworkConstantsDto,
    block: *mut BlockHandle,
    roots_hashes: *const HashRootPair,
    roots_hashes_count: usize,
) -> *mut MessageHandle {
    create_message_handle2(constants, || {
        if !block.is_null() {
            let block = (*block).deref().deref().clone();
            Message::ConfirmReq(ConfirmReq {
                block: Some(block),
                roots_hashes: Vec::new(),
            })
        } else {
            let dtos = std::slice::from_raw_parts(roots_hashes, roots_hashes_count);
            let roots_hashes = dtos
                .iter()
                .map(|dto| {
                    (
                        BlockHash::from_bytes(dto.block_hash),
                        Root::from_bytes(dto.root),
                    )
                })
                .collect();
            Message::ConfirmReq(ConfirmReq {
                block: None,
                roots_hashes,
            })
        }
    })
}

#[no_mangle]
pub extern "C" fn rsn_message_confirm_req_clone(handle: &MessageHandle) -> *mut MessageHandle {
    message_handle_clone(handle)
}

unsafe fn get_payload(handle: &MessageHandle) -> &ConfirmReq {
    let Message::ConfirmReq(payload) = &handle.message else {
        panic!("not a confirm_req_payload")
    };
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_block(handle: &MessageHandle) -> *mut BlockHandle {
    match &get_payload(handle).block {
        Some(block) => Box::into_raw(Box::new(BlockHandle(Arc::new(block.clone())))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_roots_hashes_count(
    handle: &MessageHandle,
) -> usize {
    get_payload(handle).roots_hashes.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_roots_hashes(
    handle: &MessageHandle,
    result: *mut HashRootPair,
) {
    let payload = get_payload(handle);
    let result_slice = std::slice::from_raw_parts_mut(result, payload.roots_hashes.len());
    for (i, (hash, root)) in payload.roots_hashes.iter().enumerate() {
        result_slice[i] = HashRootPair {
            block_hash: *hash.as_bytes(),
            root: *root.as_bytes(),
        };
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_equals(
    handle_a: &MessageHandle,
    handle_b: &MessageHandle,
) -> bool {
    handle_a.deref() == handle_b.deref()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_roots_string(
    handle: &MessageHandle,
    result: *mut StringDto,
) {
    (*result) = get_payload(handle).roots_string().into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_to_string(
    handle: &MessageHandle,
    result: *mut StringDto,
) {
    (*result) = handle.message.to_string().into();
}
