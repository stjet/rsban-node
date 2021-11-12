use num::FromPrimitive;
use std::ffi::c_void;

use crate::{
    blocks::{LazyBlockHash, SendBlock, SendHashables},
    numbers::{Account, Amount, BlockHash, PublicKey, RawKey, Signature},
};

use crate::ffi::{
    property_tree::{FfiPropertyTreeReader, FfiPropertyTreeWriter},
    FfiStream,
};

#[repr(C)]
pub struct SendBlockDto {
    pub previous: [u8; 32],
    pub destination: [u8; 32],
    pub balance: [u8; 16],
    pub signature: [u8; 64],
    pub work: u64,
}

#[repr(C)]
pub struct SendBlockDto2 {
    pub previous: [u8; 32],
    pub destination: [u8; 32],
    pub balance: [u8; 16],
    pub priv_key: [u8; 32],
    pub pub_key: [u8; 32],
    pub work: u64,
}

pub struct SendBlockHandle {
    pub block: SendBlock,
}

#[no_mangle]
pub extern "C" fn rsn_send_block_create(dto: &SendBlockDto) -> *mut SendBlockHandle {
    Box::into_raw(Box::new(SendBlockHandle {
        block: SendBlock::from(dto),
    }))
}

#[no_mangle]
pub extern "C" fn rsn_send_block_create2(dto: &SendBlockDto2) -> *mut SendBlockHandle {
    let previous = BlockHash::from_bytes(dto.previous);
    let destination = Account::from_bytes(dto.destination);
    let balance = Amount::from_be_bytes(dto.balance);
    let private_key = RawKey::from_bytes(dto.priv_key);
    let public_key = PublicKey::from_bytes(dto.pub_key);
    let block = match SendBlock::new(
        &previous,
        &destination,
        &balance,
        &private_key,
        &public_key,
        dto.work,
    ) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("could not create send block: {}", e);
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(SendBlockHandle { block }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_destroy(handle: *mut SendBlockHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub extern "C" fn rsn_send_block_clone(handle: &SendBlockHandle) -> *mut SendBlockHandle {
    Box::into_raw(Box::new(SendBlockHandle {
        block: handle.block.clone(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_serialize(
    handle: *mut SendBlockHandle,
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
pub unsafe extern "C" fn rsn_send_block_deserialize(stream: *mut c_void) -> *mut SendBlockHandle {
    let mut stream = FfiStream::new(stream);
    match SendBlock::deserialize(&mut stream) {
        Ok(block) => Box::into_raw(Box::new(SendBlockHandle { block })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rsn_send_block_work(handle: &SendBlockHandle) -> u64 {
    handle.block.work
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_work_set(handle: *mut SendBlockHandle, work: u64) {
    (*handle).block.work = work;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_signature(handle: &SendBlockHandle, result: *mut [u8; 64]) {
    (*result) = (*handle).block.signature.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_signature_set(
    handle: *mut SendBlockHandle,
    signature: &[u8; 64],
) {
    (*handle).block.signature = Signature::from_bytes(*signature);
}

#[no_mangle]
pub extern "C" fn rsn_send_block_equals(a: &SendBlockHandle, b: &SendBlockHandle) -> bool {
    a.block.eq(&b.block)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_zero(handle: *mut SendBlockHandle) {
    (*handle).block.zero();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_destination(
    handle: &SendBlockHandle,
    result: *mut [u8; 32],
) {
    (*result) = handle.block.hashables.destination.to_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_destination_set(
    handle: *mut SendBlockHandle,
    destination: &[u8; 32],
) {
    let destination = Account::from_bytes(*destination);
    (*handle).block.set_destination(destination);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_previous(handle: &SendBlockHandle, result: *mut [u8; 32]) {
    (*result) = handle.block.hashables.previous.to_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_previous_set(
    handle: *mut SendBlockHandle,
    previous: &[u8; 32],
) {
    let previous = BlockHash::from_bytes(*previous);
    (*handle).block.set_previous(previous);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_balance(handle: &SendBlockHandle, result: *mut [u8; 16]) {
    (*result) = handle.block.hashables.balance.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_balance_set(
    handle: *mut SendBlockHandle,
    balance: &[u8; 16],
) {
    let balance = Amount::from_be_bytes(*balance);
    (*handle).block.set_balance(balance);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_hash(handle: &SendBlockHandle, hash: *mut [u8; 32]) {
    (*hash) = handle.block.hash().to_bytes();
}

#[no_mangle]
pub extern "C" fn rsn_send_block_valid_predecessor(block_type: u8) -> bool {
    if let Some(block_type) = FromPrimitive::from_u8(block_type) {
        SendBlock::valid_predecessor(block_type)
    } else {
        false
    }
}

#[no_mangle]
pub extern "C" fn rsn_send_block_size() -> usize {
    SendBlock::serialized_size()
}

#[no_mangle]
pub extern "C" fn rsn_send_block_serialize_json(
    handle: &SendBlockHandle,
    ptree: *mut c_void,
) -> i32 {
    let mut writer = FfiPropertyTreeWriter::new(ptree);
    match handle.block.serialize_json(&mut writer) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn rsn_send_block_deserialize_json(ptree: *const c_void) -> *mut SendBlockHandle {
    let reader = FfiPropertyTreeReader::new(ptree);
    match SendBlock::deserialize_json(&reader) {
        Ok(block) => Box::into_raw(Box::new(SendBlockHandle { block })),
        Err(_) => std::ptr::null_mut(),
    }
}

impl From<&SendBlockDto> for SendBlock {
    fn from(value: &SendBlockDto) -> Self {
        SendBlock {
            hashables: SendHashables::from(value),
            signature: Signature::from_bytes(value.signature),
            work: value.work,
            hash: LazyBlockHash::new(),
            sideband: None,
        }
    }
}

impl From<&SendBlockDto> for SendHashables {
    fn from(value: &SendBlockDto) -> Self {
        SendHashables {
            previous: BlockHash::from_bytes(value.previous),
            destination: Account::from_bytes(value.destination),
            balance: Amount::new(u128::from_be_bytes(value.balance)),
        }
    }
}
