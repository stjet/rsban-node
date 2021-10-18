use num::FromPrimitive;

use crate::{
    bandwidth_limiter::BandwidthLimiter, block_details::BlockDetails,
    block_sideband::BlockSideband, epoch::Epoch,
};
use std::{convert::TryFrom, ffi::c_void, sync::Mutex};

type WriteU8Callback = unsafe extern "C" fn(*mut c_void, *const u8) -> i32;
type ReadU8Callback = unsafe extern "C" fn(*mut c_void, *mut u8) -> i32;

static mut WRITE_U8_CALLBACK: Option<WriteU8Callback> = None;
static mut READ_U8_CALLBACK: Option<ReadU8Callback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_write_u8(f: WriteU8Callback) {
    WRITE_U8_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_read_u8(f: ReadU8Callback) {
    READ_U8_CALLBACK = Some(f);
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
    match WRITE_U8_CALLBACK {
        Some(f) => match BlockDetails::try_from(dto) {
            Ok(details) => {
                let packed = details.packed();
                f(stream, &packed)
            }
            Err(_) => -1,
        },
        None => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_details_deserialize(
    dto: *mut BlockDetailsDto,
    stream: *mut c_void,
) -> i32 {
    match READ_U8_CALLBACK {
        Some(f) => {
            let mut value = 0u8;
            let raw_value = &mut value as *mut u8;
            if f(stream, raw_value) == 0 {
                if let Some(details) = BlockDetails::unpack(value) {
                    set_block_details_dto(details, dto);
                    return 0;
                }
            }

            -1
        }
        None => -1,
    }
}

unsafe fn set_block_details_dto(details: BlockDetails, result: *mut BlockDetailsDto) {
    (*result).epoch = details.epoch as u8;
    (*result).is_send = details.is_send;
    (*result).is_receive = details.is_receive;
    (*result).is_epoch = details.is_epoch;
}

#[repr(C)]
pub struct BlockSidebandDto {
    pub source_epoch: u8,
    pub height: u64,
    pub timestamp: u64,
    pub details: BlockDetailsDto,
}

#[no_mangle]
pub extern "C" fn rsn_block_sideband_foo(dto: &BlockSidebandDto) {}

impl TryFrom<&BlockDetailsDto> for BlockDetails {
    type Error = ();

    fn try_from(value: &BlockDetailsDto) -> Result<Self, Self::Error> {
        let epoch = match FromPrimitive::from_u8(value.epoch) {
            Some(e) => e,
            None => return Err(()),
        };
        let details = BlockDetails::new(epoch, value.is_send, value.is_receive, value.is_epoch);
        Ok(details)
    }
}
