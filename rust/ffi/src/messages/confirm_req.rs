use std::{ops::Deref, sync::Arc};

use crate::{core::BlockHandle, NetworkConstantsDto, StringDto};
use rsnano_node::messages::{ConfirmReqPayload, MessageEnum, Payload};

use super::{
    create_message_handle3, downcast_message, downcast_message_mut, message_handle_clone,
    MessageHandle,
};
use num_traits::FromPrimitive;
use rsnano_core::{BlockHash, BlockType, Root};

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
    create_message_handle3(constants, |protocol_info| {
        if !block.is_null() {
            let block = Arc::clone((*block).deref());
            MessageEnum::new_confirm_req_with_block(protocol_info, block)
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
            MessageEnum::new_confirm_req_with_roots_hashes(protocol_info, roots_hashes)
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<MessageEnum>(handle)
}

unsafe fn get_payload(handle: *mut MessageHandle) -> &'static ConfirmReqPayload {
    let msg = &downcast_message::<MessageEnum>(handle);
    let Payload::ConfirmReq(payload) = &msg.payload else {panic!("not a confirm_req_payload")};
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_block(
    handle: *mut MessageHandle,
) -> *mut BlockHandle {
    match &get_payload(handle).block {
        Some(block) => Box::into_raw(Box::new(BlockHandle(Arc::clone(block)))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_roots_hashes_count(
    handle: *mut MessageHandle,
) -> usize {
    get_payload(handle).roots_hashes.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_roots_hashes(
    handle: *mut MessageHandle,
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
    handle_a: *mut MessageHandle,
    handle_b: *mut MessageHandle,
) -> bool {
    let a = downcast_message_mut::<MessageEnum>(handle_a);
    let b = downcast_message_mut::<MessageEnum>(handle_b);
    a == b
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_roots_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    (*result) = get_payload(handle).roots_string().into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_size(block_type: u8, count: usize) -> usize {
    ConfirmReqPayload::serialized_size(BlockType::from_u8(block_type).unwrap(), count as u8)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_to_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    (*result) = downcast_message_mut::<MessageEnum>(handle)
        .to_string()
        .into();
}
