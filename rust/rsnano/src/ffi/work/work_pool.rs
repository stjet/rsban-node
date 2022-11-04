use std::time::Duration;

use crate::{
    config::NetworkConstants,
    ffi::NetworkConstantsDto,
    work::{WorkPool, WorkTicket},
};

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

pub struct WorkTicketHandle(WorkTicket<'static>);

#[no_mangle]
pub unsafe extern "C" fn rsn_work_pool_create_work_ticket(
    handle: *mut WorkPoolHandle,
) -> *mut WorkTicketHandle {
    let ticket = (*handle).0.create_work_ticket();
    Box::into_raw(Box::new(WorkTicketHandle(ticket)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_ticket_create() -> *mut WorkTicketHandle {
    Box::into_raw(Box::new(WorkTicketHandle(WorkTicket::never_expires())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_ticket_clone(
    handle: *mut WorkTicketHandle,
) -> *mut WorkTicketHandle {
    Box::into_raw(Box::new(WorkTicketHandle((*handle).0.clone())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_ticket_destroy(handle: *mut WorkTicketHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_ticket_expired(handle: *mut WorkTicketHandle) -> bool {
    (*handle).0.expired()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_pool_expire_work_tickets(handle: *mut WorkPoolHandle) {
    (*handle).0.expire_tickets();
}
