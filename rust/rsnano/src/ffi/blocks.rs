use super::{blake2b::FfiBlake2b, FfiStream};
use crate::{
    block_details::BlockDetails,
    blocks::{BlockSideband, BlockType, SendBlock, SendHashables},
    epoch::Epoch,
    numbers::{Account, Amount, BlockHash, Signature},
};
use num::FromPrimitive;
use std::{convert::TryFrom, ffi::c_void};

#[repr(C)]
pub struct BlockDetailsDto {
    pub epoch: u8,
    pub is_send: bool,
    pub is_receive: bool,
    pub is_epoch: bool,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_details_create(
    epoch: u8,
    is_send: bool,
    is_receive: bool,
    is_epoch: bool,
    result: *mut BlockDetailsDto,
) -> i32 {
    let epoch = match FromPrimitive::from_u8(epoch) {
        Some(e) => e,
        None => return -1,
    };

    let details = BlockDetails::new(epoch, is_send, is_receive, is_epoch);
    set_block_details_dto(&details, result);
    0
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_details_serialize(
    dto: &BlockDetailsDto,
    stream: *mut c_void,
) -> i32 {
    if let Ok(details) = BlockDetails::try_from(dto) {
        let mut stream = FfiStream::new(stream);
        if details.serialize(&mut stream).is_ok() {
            return 0;
        }
    }
    -1
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_details_deserialize(
    dto: *mut BlockDetailsDto,
    stream: *mut c_void,
) -> i32 {
    let mut stream = FfiStream::new(stream);
    if let Ok(details) = BlockDetails::deserialize(&mut stream) {
        set_block_details_dto(&details, dto);
        return 0;
    }

    -1
}

unsafe fn set_block_details_dto(details: &BlockDetails, result: *mut BlockDetailsDto) {
    (*result).epoch = details.epoch as u8;
    (*result).is_send = details.is_send;
    (*result).is_receive = details.is_receive;
    (*result).is_epoch = details.is_epoch;
}

#[repr(C)]
pub struct BlockSidebandDto {
    pub height: u64,
    pub timestamp: u64,
    pub successor: [u8; 32],
    pub account: [u8; 32],
    pub balance: [u8; 16],
    pub details: BlockDetailsDto,
    pub source_epoch: u8,
}

unsafe fn set_block_sideband_dto(sideband: &BlockSideband, result: *mut BlockSidebandDto) {
    (*result).height = sideband.height;
    (*result).timestamp = sideband.timestamp;
    (*result).successor = sideband.successor.to_be_bytes();
    (*result).account = sideband.account.to_be_bytes();
    (*result).balance = sideband.balance.to_be_bytes();
    let details_ptr: *mut BlockDetailsDto = &mut (*result).details;
    set_block_details_dto(&sideband.details, details_ptr);
    (*result).source_epoch = sideband.source_epoch as u8;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_sideband_size(block_type: u8, result: *mut i32) -> usize {
    let mut result_code = 0;
    let mut size = 0;
    if let Ok(block_type) = BlockType::try_from(block_type) {
        size = BlockSideband::serialized_size(block_type);
    } else {
        result_code = -1;
    }

    if !result.is_null() {
        *result = result_code;
    }

    size
}

#[no_mangle]
pub extern "C" fn rsn_block_sideband_serialize(
    dto: &BlockSidebandDto,
    stream: *mut c_void,
    block_type: u8,
) -> i32 {
    if let Ok(block_type) = BlockType::try_from(block_type) {
        if let Ok(sideband) = BlockSideband::try_from(dto) {
            let mut stream = FfiStream::new(stream);
            if sideband.serialize(&mut stream, block_type).is_ok() {
                return 0;
            }
        }
    }

    -1
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_sideband_deserialize(
    dto: *mut BlockSidebandDto,
    stream: *mut c_void,
    block_type: u8,
) -> i32 {
    if let Ok(block_type) = BlockType::try_from(block_type) {
        if let Ok(mut sideband) = BlockSideband::try_from(dto.as_ref().unwrap()) {
            let mut stream = FfiStream::new(stream);
            if sideband.deserialize(&mut stream, block_type).is_ok() {
                set_block_sideband_dto(&sideband, dto);
                return 0;
            }
        }
    }

    -1
}

#[repr(C)]
pub struct SendBlockDto {
    pub previous: [u8; 32],
    pub destination: [u8; 32],
    pub balance: [u8; 16],
    pub signature: [u8; 64],
    pub work: u64,
}

pub struct SendBlockHandle {
    block: SendBlock,
}

#[no_mangle]
pub extern "C" fn rsn_send_block_create(dto: &SendBlockDto) -> *mut SendBlockHandle {
    Box::into_raw(Box::new(SendBlockHandle {
        block: SendBlock::from(dto),
    }))
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
pub unsafe extern "C" fn rsn_send_block_deserialize(
    handle: *mut SendBlockHandle,
    stream: *mut c_void,
) -> i32 {
    let mut stream = FfiStream::new(stream);
    if (*handle).block.deserialize(&mut stream).is_ok() {
        0
    } else {
        -1
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
    (*handle).block.signature = Signature::from_be_bytes(*signature);
}

#[no_mangle]
pub extern "C" fn rsn_send_block_equals(a: &SendBlockHandle, b: &SendBlockHandle) -> bool {
    a.block.work.eq(&b.block.work)
        && a.block.signature.eq(&b.block.signature)
        && a.block.hashables.eq(&b.block.hashables)
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
    (*result) = handle.block.hashables.destination.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_destination_set(
    handle: *mut SendBlockHandle,
    destination: &[u8; 32],
) {
    let destination = Account::from_be_bytes(*destination);
    (*handle).block.set_destination(destination);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_previous(handle: &SendBlockHandle, result: *mut [u8; 32]) {
    (*result) = handle.block.hashables.previous.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_block_previous_set(
    handle: *mut SendBlockHandle,
    previous: &[u8; 32],
) {
    let previous = BlockHash::from_be_bytes(*previous);
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
pub extern "C" fn rsn_send_block_hash(handle: &SendBlockHandle, state: *mut c_void) -> i32 {
    let mut blake2b = FfiBlake2b::new(state);
    if handle.block.hash(&mut blake2b).is_ok() {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn rsn_send_block_valid_predecessor(block_type: u8) -> bool {
    if let Some(block_type) = FromPrimitive::from_u8(block_type){
        SendBlock::valid_predecessor(block_type)
    } else{
        false
    }
}

#[no_mangle]
pub extern "C" fn rsn_send_block_size() -> usize {
    SendBlock::serialized_size()
}

impl TryFrom<&BlockSidebandDto> for BlockSideband {
    type Error = anyhow::Error;

    fn try_from(value: &BlockSidebandDto) -> Result<Self, Self::Error> {
        let account = Account::from_be_bytes(value.account);
        let successor = BlockHash::from_be_bytes(value.successor);
        let balance = Amount::new(u128::from_be_bytes(value.balance));
        let details = BlockDetails::try_from(&value.details)?;
        let source_epoch = Epoch::try_from(value.source_epoch)?;
        let sideband = BlockSideband::new(
            account,
            successor,
            balance,
            value.height,
            value.timestamp,
            details,
            source_epoch,
        );
        Ok(sideband)
    }
}

impl TryFrom<&BlockDetailsDto> for BlockDetails {
    type Error = anyhow::Error;

    fn try_from(value: &BlockDetailsDto) -> Result<Self, Self::Error> {
        let epoch = Epoch::try_from(value.epoch)?;
        let details = BlockDetails::new(epoch, value.is_send, value.is_receive, value.is_epoch);
        Ok(details)
    }
}

impl TryFrom<u8> for Epoch {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        FromPrimitive::from_u8(value).ok_or_else(|| anyhow!("invalid epoch value"))
    }
}

impl TryFrom<u8> for BlockType {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        FromPrimitive::from_u8(value).ok_or_else(|| anyhow!("invalid block type value"))
    }
}

impl From<&SendBlockDto> for SendBlock {
    fn from(value: &SendBlockDto) -> Self {
        SendBlock {
            hashables: SendHashables::from(value),
            signature: Signature::from_be_bytes(value.signature),
            work: value.work,
        }
    }
}

impl From<&SendBlockDto> for SendHashables {
    fn from(value: &SendBlockDto) -> Self {
        SendHashables {
            previous: BlockHash::from_be_bytes(value.previous),
            destination: Account::from_be_bytes(value.destination),
            balance: Amount::new(u128::from_be_bytes(value.balance)),
        }
    }
}
