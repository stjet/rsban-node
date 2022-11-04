use std::time::Duration;

use crate::{config::NetworkConstants, ffi::NetworkConstantsDto, work::WorkPool};

pub struct WorkPoolHandle(WorkPool);

#[no_mangle]
pub unsafe extern "C" fn rsn_work_pool_create(
    network_constants: *const NetworkConstantsDto,
    max_threads: u32,
    pow_rate_limiter_ns: u64,
) -> *mut WorkPoolHandle {
    let network_constants = NetworkConstants::try_from(&*network_constants).unwrap();
    Box::into_raw(Box::new(WorkPoolHandle(WorkPool::new(
        network_constants,
        max_threads,
        Duration::from_nanos(pow_rate_limiter_ns),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_pool_destroy(handle: *mut WorkPoolHandle) {
    drop(Box::from_raw(handle));
}
