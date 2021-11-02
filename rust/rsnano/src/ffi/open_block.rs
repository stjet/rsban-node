use std::ffi::c_void;

use crate::{
    blocks::{OpenBlock, OpenHashables},
    numbers::{Account, BlockHash, Signature},
};

use super::{blake2b::FfiBlake2b, FfiStream};

pub struct OpenBlockHandle {
    block: OpenBlock,
}

#[repr(C)]
pub struct OpenBlockDto {
    pub work: u64,
    pub signature: [u8; 64],
    pub source: [u8; 32],
    pub representative: [u8; 32],
    pub account: [u8; 32],
}

#[no_mangle]
pub extern "C" fn rsn_open_block_create(dto: &OpenBlockDto) -> *mut OpenBlockHandle {
    Box::into_raw(Box::new(OpenBlockHandle {
        block: OpenBlock {
            work: dto.work,
            signature: Signature::from_bytes(dto.signature),
            hashables: OpenHashables {
                source: BlockHash::from_be_bytes(dto.source),
                representative: Account::from_be_bytes(dto.representative),
                account: Account::from_be_bytes(dto.account),
            },
        },
    }))
}

#[no_mangle]
pub extern "C" fn rsn_open_block_clone(handle: &OpenBlockHandle) -> *mut OpenBlockHandle {
    Box::into_raw(Box::new(OpenBlockHandle {
        block: handle.block.clone(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_destroy(handle: *mut OpenBlockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_work_set(handle: *mut OpenBlockHandle, work: u64) {
    (*handle).block.work = work;
}

#[no_mangle]
pub extern "C" fn rsn_open_block_work(handle: &OpenBlockHandle) -> u64 {
    handle.block.work
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_signature(handle: &OpenBlockHandle, result: *mut [u8; 64]) {
    (*result) = (*handle).block.signature.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_signature_set(
    handle: *mut OpenBlockHandle,
    signature: &[u8; 64],
) {
    (*handle).block.signature = Signature::from_bytes(*signature);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_source(handle: &OpenBlockHandle, result: *mut [u8; 32]) {
    (*result) = (*handle).block.hashables.source.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_source_set(
    handle: *mut OpenBlockHandle,
    source: &[u8; 32],
) {
    (*handle).block.hashables.source = BlockHash::from_be_bytes(*source);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_representative(
    handle: &OpenBlockHandle,
    result: *mut [u8; 32],
) {
    (*result) = (*handle).block.hashables.representative.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_representative_set(
    handle: *mut OpenBlockHandle,
    representative: &[u8; 32],
) {
    (*handle).block.hashables.representative = Account::from_be_bytes(*representative);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_account(handle: &OpenBlockHandle, result: *mut [u8; 32]) {
    (*result) = (*handle).block.hashables.account.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_account_set(
    handle: *mut OpenBlockHandle,
    account: &[u8; 32],
) {
    (*handle).block.hashables.account = Account::from_be_bytes(*account);
}

#[no_mangle]
pub extern "C" fn rsn_open_block_equals(a: &OpenBlockHandle, b: &OpenBlockHandle) -> bool {
    a.block.work.eq(&b.block.work)
        && a.block.signature.eq(&b.block.signature)
        && a.block.hashables.eq(&b.block.hashables)
}

#[no_mangle]
pub extern "C" fn rsn_open_block_size() -> usize {
    OpenBlock::serialized_size()
}

#[no_mangle]
pub extern "C" fn rsn_open_block_hash(handle: &OpenBlockHandle, state: *mut c_void) -> i32 {
    let mut blake2b = FfiBlake2b::new(state);
    if handle.block.hash(&mut blake2b).is_ok() {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_serialize(
    handle: *mut OpenBlockHandle,
    stream: *mut c_void,
) -> i32 {
    let mut stream = FfiStream::new(stream);
    if (*handle).block.serialize(&mut stream).is_ok() {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_deserialize(
    handle: *mut OpenBlockHandle,
    stream: *mut c_void,
) -> i32 {
    let mut stream = FfiStream::new(stream);
    if (*handle).block.deserialize(&mut stream).is_ok() {
        0
    } else {
        -1
    }
}
