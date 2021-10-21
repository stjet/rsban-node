use num::FromPrimitive;
use primitive_types::U256;

use crate::{
    bandwidth_limiter::BandwidthLimiter,
    block_details::BlockDetails,
    block_sideband::{Account, Amount, BlockHash, BlockSideband, BlockType, PublicKey},
    epoch::Epoch,
    utils::Stream,
};
use std::{convert::TryFrom, ffi::c_void, sync::Mutex};

type WriteU8Callback = unsafe extern "C" fn(*mut c_void, u8) -> i32;
type WriteBytesCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
type ReadU8Callback = unsafe extern "C" fn(*mut c_void, *mut u8) -> i32;

static mut WRITE_U8_CALLBACK: Option<WriteU8Callback> = None;
static mut WRITE_BYTES_CALLBACK: Option<WriteBytesCallback> = None;
static mut READ_U8_CALLBACK: Option<ReadU8Callback> = None;

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
        if bytes.len() != 32 {
            bail!("not implemented yet")
        }

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
    set_block_details_dto(details, result);
    return 0;
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
        set_block_details_dto(details, dto);
        return 0;
    }

    return -1;
}

unsafe fn set_block_details_dto(details: BlockDetails, result: *mut BlockDetailsDto) {
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

#[no_mangle]
pub extern "C" fn rsn_block_sideband_size(block_type: u8, result: *mut i32) -> usize {
    let mut result_code = 0;
    let mut size = 0;
    if let Ok(block_type) = BlockType::try_from(block_type) {
        size = BlockSideband::serialized_size(block_type);
    } else {
        result_code = -1;
    }

    if !result.is_null() {
        unsafe {
            *result = result_code;
        }
    }

    size
}

#[no_mangle]
pub extern "C" fn rsn_block_sideband_serialize(_dto: &BlockSidebandDto, _stream: *mut c_void) -> i32 {
    0
}

impl TryFrom<&BlockSidebandDto> for BlockSideband {
    type Error = anyhow::Error;

    fn try_from(value: &BlockSidebandDto) -> Result<Self, Self::Error> {
        let pub_key = PublicKey::new(U256::from_big_endian(&value.account));
        let account = Account::new(pub_key);
        let successor = BlockHash::new(U256::from_big_endian(&value.successor));
        let balance = Amount::new(u128::from_be_bytes(value.balance));
        let details = BlockDetails::try_from(&value.details)?;
        let source_epoch = Epoch::try_from(value.source_epoch)?;
        let sideband = BlockSideband::new(
            account,
            successor,
            balance,
            value.height,
            value.height,
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
