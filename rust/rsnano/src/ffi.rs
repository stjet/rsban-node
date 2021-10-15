use num::FromPrimitive;

use crate::{bandwidth_limiter::BandwidthLimiter, block_details::BlockDetails, block_sideband::BlockSideband, epoch::Epoch};
use std::sync::Mutex;

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
) -> bool {
    limiter.limiter.lock().unwrap().should_drop(message_size)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bandwidth_limiter_reset(
    limiter: &BandwidthLimiterHandle,
    limit_burst_ration: f64,
    limit: usize,
) {
    limiter
        .limiter
        .lock()
        .unwrap()
        .reset(limit_burst_ration, limit)
}


#[repr(C)]
pub struct BlockDetailsDto {
    pub epoch: u8,
    pub is_send: bool,
    pub is_receive: bool,
    pub is_epoch: bool,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_details_create(epoch: u8, is_send: bool, is_receive: bool,  is_epoch: bool, result: *mut BlockDetailsDto) {
    let epoch = FromPrimitive::from_u8(epoch).unwrap();
    let details = BlockDetails::new(epoch, is_send, is_receive, is_epoch);
    set_block_details_dto(details, result);
}

#[no_mangle]
pub extern "C" fn rsn_block_details_packed(details: &BlockDetailsDto) -> u8{
    let epoch = FromPrimitive::from_u8(details.epoch).unwrap();
    let details = BlockDetails::new(epoch, details.is_send, details.is_receive, details.is_epoch);
    details.packed()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_details_unpack(data: u8, result: *mut BlockDetailsDto){
    let details = BlockDetails::unpack(data);
    set_block_details_dto(details, result);
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
}

#[no_mangle]
pub extern "C" fn rsn_block_sideband_foo(dto: &BlockSidebandDto){

}