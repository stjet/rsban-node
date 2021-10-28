use std::ffi::c_void;

use crate::{
    blocks::{ChangeBlock, ChangeHashables},
    numbers::{Account, BlockHash, Signature},
};

use super::{blake2b::FfiBlake2b, FfiStream};

pub struct ChangeBlockHandle {
    block: ChangeBlock,
}

#[repr(C)]
pub struct ChangeBlockDto {
    pub work: u64,
    pub signature: [u8; 64],
    pub previous: [u8; 32],
    pub representative: [u8; 32],
}

#[no_mangle]
pub extern "C" fn rsn_change_block_create(dto: &ChangeBlockDto) -> *mut ChangeBlockHandle {
    Box::into_raw(Box::new(ChangeBlockHandle {
        block: ChangeBlock {
            work: dto.work,
            signature: Signature::from_be_bytes(dto.signature),
            hashables: ChangeHashables {
                previous: BlockHash::from_be_bytes(dto.previous),
                representative: Account::from_be_bytes(dto.representative),
            },
        },
    }))
}

#[no_mangle]
pub extern "C" fn rsn_change_block_clone(handle: &ChangeBlockHandle) -> *mut ChangeBlockHandle {
    Box::into_raw(Box::new(ChangeBlockHandle {
        block: handle.block.clone(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_destroy(handle: *mut ChangeBlockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_work_set(handle: *mut ChangeBlockHandle, work: u64) {
    (*handle).block.work = work;
}

#[no_mangle]
pub extern "C" fn rsn_change_block_work(handle: &ChangeBlockHandle) -> u64 {
    handle.block.work
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_signature(
    handle: &ChangeBlockHandle,
    result: *mut [u8; 64],
) {
    (*result) = (*handle).block.signature.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_signature_set(
    handle: *mut ChangeBlockHandle,
    signature: &[u8; 64],
) {
    (*handle).block.signature = Signature::from_be_bytes(*signature);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_previous(
    handle: &ChangeBlockHandle,
    result: *mut [u8; 32],
) {
    (*result) = (*handle).block.hashables.previous.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_previous_set(
    handle: *mut ChangeBlockHandle,
    source: &[u8; 32],
) {
    (*handle).block.hashables.previous = BlockHash::from_be_bytes(*source);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_representative(
    handle: &ChangeBlockHandle,
    result: *mut [u8; 32],
) {
    (*result) = (*handle).block.hashables.representative.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_representative_set(
    handle: *mut ChangeBlockHandle,
    representative: &[u8; 32],
) {
    (*handle).block.hashables.representative = Account::from_be_bytes(*representative);
}

#[no_mangle]
pub extern "C" fn rsn_change_block_equals(a: &ChangeBlockHandle, b: &ChangeBlockHandle) -> bool {
    a.block.work.eq(&b.block.work)
        && a.block.signature.eq(&b.block.signature)
        && a.block.hashables.eq(&b.block.hashables)
}

#[no_mangle]
pub extern "C" fn rsn_change_block_size() -> usize {
    ChangeBlock::serialized_size()
}

#[no_mangle]
pub extern "C" fn rsn_change_block_hash(handle: &ChangeBlockHandle, state: *mut c_void) -> i32 {
    let mut blake2b = FfiBlake2b::new(state);
    if handle.block.hash(&mut blake2b).is_ok() {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_serialize(
    handle: *mut ChangeBlockHandle,
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
pub unsafe extern "C" fn rsn_change_block_deserialize(
    handle: *mut ChangeBlockHandle,
    stream: *mut c_void,
) -> i32 {
    let mut stream = FfiStream::new(stream);
    if (*handle).block.deserialize(&mut stream).is_ok() {
        0
    } else {
        -1
    }
}
