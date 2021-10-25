use num::FromPrimitive;

use crate::blocks::{BlockSideband, BlockType, SendBlock};
use crate::numbers::Signature;
use crate::{
    bandwidth_limiter::BandwidthLimiter,
    block_details::BlockDetails,
    blocks::SendHashables,
    epoch::Epoch,
    numbers::{Account, Amount, BlockHash, PublicKey},
    utils::Stream,
};
use std::{convert::TryFrom, ffi::c_void, sync::Mutex};

type WriteU8Callback = unsafe extern "C" fn(*mut c_void, u8) -> i32;
type WriteBytesCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
type ReadU8Callback = unsafe extern "C" fn(*mut c_void, *mut u8) -> i32;
type ReadBytesCallback = unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> i32;

static mut WRITE_U8_CALLBACK: Option<WriteU8Callback> = None;
static mut WRITE_BYTES_CALLBACK: Option<WriteBytesCallback> = None;
static mut READ_U8_CALLBACK: Option<ReadU8Callback> = None;
static mut READ_BYTES_CALLBACK: Option<ReadBytesCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_write_u8(f: WriteU8Callback) {
    WRITE_U8_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_write_bytes(f: WriteBytesCallback) {
    WRITE_BYTES_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_read_u8(f: ReadU8Callback) {
    READ_U8_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_read_bytes(f: ReadBytesCallback) {
    READ_BYTES_CALLBACK = Some(f);
}

struct FfiStream {
    stream_handle: *mut c_void,
}

impl FfiStream {
    fn new(stream_handle: *mut c_void) -> Self {
        Self { stream_handle }
    }
}

impl Stream for FfiStream {
    fn write_u8(&mut self, value: u8) -> anyhow::Result<()> {
        unsafe {
            match WRITE_U8_CALLBACK {
                Some(f) => {
                    let result = f(self.stream_handle, value);

                    if result == 0 {
                        Ok(())
                    } else {
                        Err(anyhow!("callback returned error"))
                    }
                }
                None => Err(anyhow!("WRITE_U8_CALLBACK missing")),
            }
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        unsafe {
            match WRITE_BYTES_CALLBACK {
                Some(f) => {
                    if f(self.stream_handle, bytes.as_ptr(), bytes.len()) == 0 {
                        Ok(())
                    } else {
                        Err(anyhow!("callback returned error"))
                    }
                }
                None => Err(anyhow!("WRITE_32_BYTES_CALLBACK missing")),
            }
        }
    }

    fn read_u8(&mut self) -> anyhow::Result<u8> {
        unsafe {
            match READ_U8_CALLBACK {
                Some(f) => {
                    let mut value = 0u8;
                    let raw_value = &mut value as *mut u8;
                    if f(self.stream_handle, raw_value) == 0 {
                        Ok(value)
                    } else {
                        Err(anyhow!("callback returned error"))
                    }
                }
                None => Err(anyhow!("READ_U8_CALLBACK missing")),
            }
        }
    }

    fn read_bytes(&mut self, buffer: &mut [u8], len: usize) -> anyhow::Result<()> {
        unsafe {
            match READ_BYTES_CALLBACK {
                Some(f) => {
                    if f(self.stream_handle, buffer.as_mut_ptr(), len) == 0 {
                        Ok(())
                    } else {
                        Err(anyhow!("callback returned error"))
                    }
                }
                None => Err(anyhow!("READ_BYTES_CALLBACK missing")),
            }
        }
    }
}

pub struct BandwidthLimiterHandle {
    limiter: Mutex<BandwidthLimiter>,
}

#[no_mangle]
pub extern "C" fn rsn_bandwidth_limiter_create(
    limit_burst_ratio: f64,
    limit: usize,
) -> *mut BandwidthLimiterHandle {
    Box::into_raw(Box::new(BandwidthLimiterHandle {
        limiter: Mutex::new(BandwidthLimiter::new(limit_burst_ratio, limit)),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bandwidth_limiter_destroy(limiter: *mut BandwidthLimiterHandle) {
    drop(Box::from_raw(limiter));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bandwidth_limiter_should_drop(
    limiter: &BandwidthLimiterHandle,
    message_size: usize,
    result: *mut i32,
) -> bool {
    match limiter.limiter.lock() {
        Ok(mut lock) => {
            if !result.is_null() {
                *result = 0;
            }
            lock.should_drop(message_size)
        }
        Err(_) => {
            if !result.is_null() {
                *result = -1;
            }
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bandwidth_limiter_reset(
    limiter: &BandwidthLimiterHandle,
    limit_burst_ratio: f64,
    limit: usize,
) -> i32 {
    match limiter.limiter.lock() {
        Ok(mut lock) => {
            lock.reset(limit_burst_ratio, limit);
            0
        }
        Err(_) => -1,
    }
}

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
pub struct SendHashablesDto {
    pub previous: [u8; 32],
    pub destination: [u8; 32],
    pub balance: [u8; 16],
}

#[no_mangle]
pub unsafe extern "C" fn rsn_send_hashables_deserialize(
    dto: *mut SendHashablesDto,
    stream: *mut c_void,
) -> i32 {
    let mut stream = FfiStream::new(stream);
    if let Ok(hashables) = SendHashables::deserialize(&mut stream) {
        set_send_hashables_dto(&hashables, dto);
        0
    } else {
        -1
    }
}

unsafe fn set_send_hashables_dto(hashables: &SendHashables, dto: *mut SendHashablesDto) {
    (*dto).previous = hashables.previous.to_be_bytes();
    (*dto).destination = hashables.destination.to_be_bytes();
    (*dto).balance = hashables.balance.to_be_bytes();
}

#[repr(C)]
pub struct SendBlockDto {
    pub hashables: SendHashablesDto,
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
    dto: &SendBlockDto,
    stream: *mut c_void,
) -> i32 {
    update_send_block(handle, dto);
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
    dto: *mut SendBlockDto,
    stream: *mut c_void,
) -> i32 {
    let mut stream = FfiStream::new(stream);
    if (*handle).block.deserialize(&mut stream).is_ok() {
        set_send_block_dto(&((*handle).block), dto);
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
    (*handle).block.signature = Signature::new(*signature);
}

#[no_mangle]
pub extern "C" fn rsn_send_block_equals(a: &SendBlockHandle, b: &SendBlockHandle) -> bool {
    a.block.work.eq(&b.block.work) && a.block.signature.eq(&b.block.signature)
}

unsafe fn update_send_block(handle: *mut SendBlockHandle, dto: &SendBlockDto) {
    (*handle).block.work = dto.work;
    (*handle).block.signature = Signature::new(dto.signature);
    (*handle).block.hashables = SendHashables::from(&dto.hashables);
}

unsafe fn set_send_block_dto(block: &SendBlock, dto: *mut SendBlockDto) {
    set_send_hashables_dto(&block.hashables, &mut (*dto).hashables);
    (*dto).signature = block.signature.to_be_bytes();
    (*dto).work = block.work;
}

impl TryFrom<&BlockSidebandDto> for BlockSideband {
    type Error = anyhow::Error;

    fn try_from(value: &BlockSidebandDto) -> Result<Self, Self::Error> {
        let pub_key = PublicKey::new(value.account);
        let account = Account::new(pub_key);
        let successor = BlockHash::new(value.successor);
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
            hashables: SendHashables::from(&value.hashables),
            signature: Signature::new(value.signature),
            work: value.work,
        }
    }
}

impl From<&SendHashablesDto> for SendHashables {
    fn from(value: &SendHashablesDto) -> Self {
        SendHashables {
            previous: BlockHash::new(value.previous),
            destination: Account::new(PublicKey::new(value.destination)),
            balance: Amount::new(u128::from_be_bytes(value.balance)),
        }
    }
}
