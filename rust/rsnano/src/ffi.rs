use crate::bandwidth_limiter::BandwidthLimiter;
use std::sync::Mutex;

#[no_mangle]
pub extern "C" fn rsn_bandwidth_limiter_create(
    limit_burst_ratio: f64,
    limit: usize,
) -> *mut Mutex<BandwidthLimiter> {
    Box::into_raw(Box::new(Mutex::new(BandwidthLimiter::new(
        limit_burst_ratio,
        limit,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bandwidth_limiter_destroy(limiter: *mut Mutex<BandwidthLimiter>) {
    drop(Box::from_raw(limiter));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bandwidth_limiter_should_drop(
    limiter: &Mutex<BandwidthLimiter>,
    message_size: usize,
) -> bool {
    limiter.lock().unwrap().should_drop(message_size)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bandwidth_limiter_reset(
    limiter: &Mutex<BandwidthLimiter>,
    limit_burst_ration: f64,
    limit: usize,
) {
    limiter.lock().unwrap().reset(limit_burst_ration, limit)
}
