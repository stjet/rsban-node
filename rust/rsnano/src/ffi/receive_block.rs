use std::ffi::c_void;

use crate::{
    blocks::{ReceiveBlock, ReceiveHashables},
    numbers::{BlockHash, Signature},
};

use super::{blake2b::FfiBlake2b, property_tree::{FfiPropertyTreeReader, FfiPropertyTreeWriter}};

pub struct ReceiveBlockHandle {
    block: ReceiveBlock,
}

#[repr(C)]
pub struct ReceiveBlockDto {
    pub work: u64,
    pub signature: [u8; 64],
    pub previous: [u8; 32],
    pub source: [u8; 32],
}

#[no_mangle]
pub extern "C" fn rsn_receive_block_create(dto: &ReceiveBlockDto) -> *mut ReceiveBlockHandle {
    Box::into_raw(Box::new(ReceiveBlockHandle {
        block: ReceiveBlock {
            work: dto.work,
            signature: Signature::from_bytes(dto.signature),
            hashables: ReceiveHashables {
                previous: BlockHash::from_bytes(dto.previous),
                source: BlockHash::from_bytes(dto.source),
            },
        },
    }))
}

#[no_mangle]
pub extern "C" fn rsn_receive_block_clone(handle: &ReceiveBlockHandle) -> *mut ReceiveBlockHandle {
    Box::into_raw(Box::new(ReceiveBlockHandle {
        block: handle.block.clone(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_block_destroy(handle: *mut ReceiveBlockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_block_work_set(handle: *mut ReceiveBlockHandle, work: u64) {
    (*handle).block.work = work;
}

#[no_mangle]
pub extern "C" fn rsn_receive_block_work(handle: &ReceiveBlockHandle) -> u64 {
    handle.block.work
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_block_signature(
    handle: &ReceiveBlockHandle,
    result: *mut [u8; 64],
) {
    (*result) = (*handle).block.signature.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_block_signature_set(
    handle: *mut ReceiveBlockHandle,
    signature: &[u8; 64],
) {
    (*handle).block.signature = Signature::from_bytes(*signature);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_block_previous(
    handle: &ReceiveBlockHandle,
    result: *mut [u8; 32],
) {
    (*result) = handle.block.hashables.previous.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_block_previous_set(
    handle: *mut ReceiveBlockHandle,
    previous: &[u8; 32],
) {
    let previous = BlockHash::from_bytes(*previous);
    (*handle).block.hashables.previous = previous;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_block_source(
    handle: &ReceiveBlockHandle,
    result: *mut [u8; 32],
) {
    (*result) = handle.block.hashables.source.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_receive_block_source_set(
    handle: *mut ReceiveBlockHandle,
    previous: &[u8; 32],
) {
    let source = BlockHash::from_bytes(*previous);
    (*handle).block.hashables.source = source;
}

#[no_mangle]
pub extern "C" fn rsn_receive_block_hash(handle: &ReceiveBlockHandle, state: *mut c_void) -> i32 {
    let mut blake2b = FfiBlake2b::new(state);
    if handle.block.hash(&mut blake2b).is_ok() {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn rsn_receive_block_equals(a: &ReceiveBlockHandle, b: &ReceiveBlockHandle) -> bool {
    a.block.work.eq(&b.block.work)
        && a.block.signature.eq(&b.block.signature)
        && a.block.hashables.eq(&b.block.hashables)
}

#[no_mangle]
pub extern "C" fn rsn_receive_block_size() -> usize {
    ReceiveBlock::serialized_size()
}

#[no_mangle]
pub extern "C" fn rsn_receive_block_serialize_json(
    handle: &ReceiveBlockHandle,
    ptree: *mut c_void,
) -> i32 {
    let mut writer = FfiPropertyTreeWriter::new(ptree);
    match handle.block.serialize_json(&mut writer) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn rsn_receive_block_deserialize_json(ptree: *const c_void) -> *mut ReceiveBlockHandle {
    let reader = FfiPropertyTreeReader::new(ptree);
    match ReceiveBlock::deserialize_json(&reader) {
        Ok(block) => Box::into_raw(Box::new(ReceiveBlockHandle { block })),
        Err(_) => std::ptr::null_mut(),
    }
}