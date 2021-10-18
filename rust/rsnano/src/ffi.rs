use num::FromPrimitive;

use crate::{bandwidth_limiter::BandwidthLimiter, block_details::BlockDetails, block_sideband::BlockSideband, epoch::Epoch};
use std::{ffi::c_void, sync::Mutex};

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_write_u8(f: WriteU8Callback){
    WRITE_U8_CALLBACK = Some(f);
}

type WriteU8Callback = unsafe extern "C" fn(*mut c_void, *const u8) -> i32;
static mut WRITE_U8_CALLBACK: Option<WriteU8Callback> = None;

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
    match limiter.limiter.lock(){
        Ok(mut lock) => {
            if !result.is_null(){
                *result = 0;
            }
            lock.should_drop(message_size)
        }
        Err(_) => {
            if !result.is_null(){
                *result = -1;
            }
            false
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bandwidth_limiter_reset(
    limiter: &BandwidthLimiterHandle,
    limit_burst_ratio: f64,
    limit: usize,
) -> i32 {
    match limiter
        .limiter
        .lock(){
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
pub unsafe extern "C" fn rsn_block_details_create(epoch: u8, is_send: bool, is_receive: bool,  is_epoch: bool, result: *mut BlockDetailsDto) -> i32 {
    let epoch = match FromPrimitive::from_u8(epoch){
        Some(e) => e,
        None => return -1,
    };

    let details = BlockDetails::new(epoch, is_send, is_receive, is_epoch);
    set_block_details_dto(details, result);
    return 0;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_details_packed(details: &BlockDetailsDto, result: *mut i32) -> u8{
    let epoch = match FromPrimitive::from_u8(details.epoch){
        Some(e) => e,
        None => {
            if !result.is_null(){
                *result = -1;
            }
            return 0;
        },
    };
    let details = BlockDetails::new(epoch, details.is_send, details.is_receive, details.is_epoch);
    if !result.is_null() {
        *result = 0;
    }
    details.packed()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_details_unpack(data: u8, result: *mut BlockDetailsDto){
    let details = BlockDetails::unpack(data);
    set_block_details_dto(details, result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_details_serialize(dto: &BlockDetailsDto, stream: *mut c_void) -> i32{
    match WRITE_U8_CALLBACK{
        Some(f) => {
            let epoch = match FromPrimitive::from_u8(dto.epoch){
                Some(e) => e,
                None => return -1,
            };
            let details = BlockDetails::new(epoch, dto.is_send, dto.is_receive, dto.is_epoch);
            let packed =details.packed(); 
            f(stream, &packed)
        },
        None => {
            -1
        }
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
    pub details: BlockDetailsDto
}

#[no_mangle]
pub extern "C" fn rsn_block_sideband_foo(dto: &BlockSidebandDto){
}

