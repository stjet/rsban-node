use std::ffi::c_void;

use crate::{
    blocks::{StateBlock, StateHashables},
    numbers::{Account, Amount, BlockHash, Link, Signature},
};

use super::{blake2b::FfiBlake2b, FfiStream};

pub struct StateBlockHandle {
    block: StateBlock,
}

#[repr(C)]
pub struct StateBlockDto {
    pub signature: [u8; 64],
    pub account: [u8; 32],
    pub previous: [u8; 32],
    pub representative: [u8; 32],
    pub link: [u8; 32],
    pub balance: [u8; 16],
    pub work: u64,
}

#[no_mangle]
pub extern "C" fn rsn_state_block_create(dto: &StateBlockDto) -> *mut StateBlockHandle {
    Box::into_raw(Box::new(StateBlockHandle {
        block: StateBlock {
            work: dto.work,
            signature: Signature::from_bytes(dto.signature),
            hashables: StateHashables {
                account: Account::from_be_bytes(dto.account),
                previous: BlockHash::from_be_bytes(dto.previous),
                representative: Account::from_be_bytes(dto.representative),
                balance: Amount::from_be_bytes(dto.balance),
                link: Link::from_be_bytes(dto.link),
            },
        },
    }))
}

#[no_mangle]
pub extern "C" fn rsn_state_block_clone(handle: &StateBlockHandle) -> *mut StateBlockHandle {
    Box::into_raw(Box::new(StateBlockHandle {
        block: handle.block.clone(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_destroy(handle: *mut StateBlockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_work_set(handle: *mut StateBlockHandle, work: u64) {
    (*handle).block.work = work;
}

#[no_mangle]
pub extern "C" fn rsn_state_block_work(handle: &StateBlockHandle) -> u64 {
    handle.block.work
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_signature(
    handle: &StateBlockHandle,
    result: *mut [u8; 64],
) {
    (*result) = (*handle).block.signature.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_signature_set(
    handle: *mut StateBlockHandle,
    signature: &[u8; 64],
) {
    (*handle).block.signature = Signature::from_bytes(*signature);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_account(handle: &StateBlockHandle, result: *mut [u8; 32]) {
    (*result) = (*handle).block.hashables.account.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_account_set(
    handle: *mut StateBlockHandle,
    source: &[u8; 32],
) {
    (*handle).block.hashables.account = Account::from_be_bytes(*source);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_previous(
    handle: &StateBlockHandle,
    result: *mut [u8; 32],
) {
    (*result) = (*handle).block.hashables.previous.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_previous_set(
    handle: *mut StateBlockHandle,
    source: &[u8; 32],
) {
    (*handle).block.hashables.previous = BlockHash::from_be_bytes(*source);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_representative(
    handle: &StateBlockHandle,
    result: *mut [u8; 32],
) {
    (*result) = (*handle).block.hashables.representative.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_representative_set(
    handle: *mut StateBlockHandle,
    representative: &[u8; 32],
) {
    (*handle).block.hashables.representative = Account::from_be_bytes(*representative);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_balance(handle: &StateBlockHandle, result: *mut [u8; 16]) {
    (*result) = (*handle).block.hashables.balance.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_balance_set(
    handle: *mut StateBlockHandle,
    balance: &[u8; 16],
) {
    (*handle).block.hashables.balance = Amount::from_be_bytes(*balance);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_link(handle: &StateBlockHandle, result: *mut [u8; 32]) {
    (*result) = (*handle).block.hashables.link.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_link_set(handle: *mut StateBlockHandle, link: &[u8; 32]) {
    (*handle).block.hashables.link = Link::from_be_bytes(*link);
}

#[no_mangle]
pub extern "C" fn rsn_state_block_equals(a: &StateBlockHandle, b: &StateBlockHandle) -> bool {
    a.block.work.eq(&b.block.work)
        && a.block.signature.eq(&b.block.signature)
        && a.block.hashables.eq(&b.block.hashables)
}

#[no_mangle]
pub extern "C" fn rsn_state_block_size() -> usize {
    StateBlock::serialized_size()
}

#[no_mangle]
pub extern "C" fn rsn_state_block_hash(handle: &StateBlockHandle, state: *mut c_void) -> i32 {
    let mut blake2b = FfiBlake2b::new(state);
    if handle.block.hash(&mut blake2b).is_ok() {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_state_block_serialize(
    handle: *mut StateBlockHandle,
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
pub unsafe extern "C" fn rsn_state_block_deserialize(
    handle: *mut StateBlockHandle,
    stream: *mut c_void,
) -> i32 {
    let mut stream = FfiStream::new(stream);
    if (*handle).block.deserialize(&mut stream).is_ok() {
        0
    } else {
        -1
    }
}
