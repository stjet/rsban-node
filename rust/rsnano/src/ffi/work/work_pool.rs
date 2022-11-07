use num_traits::FromPrimitive;
use std::{ffi::c_void, time::Duration};

use crate::{
    config::NetworkConstants,
    core::{Root, WorkVersion},
    ffi::NetworkConstantsDto,
    work::{WorkPool, WorkTicket},
};

pub struct WorkPoolHandle(WorkPool<'static>);

type OpenclCallback =
    unsafe extern "C" fn(*mut c_void, u8, *const u8, u64, *mut WorkTicketHandle, *mut u64) -> bool;

#[no_mangle]
pub unsafe extern "C" fn rsn_work_pool_create(
    network_constants: *const NetworkConstantsDto,
    max_threads: u32,
    pow_rate_limiter_ns: u64,
    opencl: OpenclCallback,
    opencl_context: *mut c_void,
    destroy_context: unsafe extern "C" fn(*mut c_void),
) -> *mut WorkPoolHandle {
    let network_constants = NetworkConstants::try_from(&*network_constants).unwrap();
    Box::into_raw(Box::new(WorkPoolHandle(WorkPool::new(
        network_constants,
        max_threads,
        Duration::from_nanos(pow_rate_limiter_ns),
        create_opencl_wrapper(opencl, opencl_context, destroy_context),
    ))))
}

struct OpenclWrapper {
    callback: OpenclCallback,
    destroy_context: unsafe extern "C" fn(*mut c_void),
    context: *mut c_void,
}

impl Drop for OpenclWrapper {
    fn drop(&mut self) {
        unsafe {
            (self.destroy_context)(self.context);
        }
    }
}

fn create_opencl_wrapper(
    opencl: OpenclCallback,
    context: *mut c_void,
    destroy_context: unsafe extern "C" fn(*mut c_void),
) -> Option<Box<dyn Fn(WorkVersion, Root, u64, WorkTicket) -> Option<u64>>> {
    if context.is_null() {
        return None;
    }

    let wrapper = OpenclWrapper {
        callback: opencl,
        destroy_context,
        context,
    };

    Some(Box::new(move |version, root, difficulty, ticket| {
        let mut work = 0;
        let ticket = unsafe { std::mem::transmute::<WorkTicket, WorkTicket<'static>>(ticket) };
        let ticket_handle = Box::into_raw(Box::new(WorkTicketHandle(ticket)));
        let found = unsafe {
            (wrapper.callback)(
                wrapper.context,
                version as u8,
                root.as_bytes().as_ptr(),
                difficulty,
                ticket_handle,
                &mut work,
            )
        };
        if found {
            Some(work)
        } else {
            None
        }
    }))
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

#[no_mangle]
pub unsafe extern "C" fn rsn_work_pool_call_open_cl(
    handle: *mut WorkPoolHandle,
    version: u8,
    root: *const u8,
    difficulty: u64,
    ticket: *mut WorkTicketHandle,
    result: *mut u64,
) -> bool {
    let work = (*handle).0.call_open_cl(
        WorkVersion::from_u8(version).unwrap(),
        Root::from_ptr(root),
        difficulty,
        (*ticket).0.clone(),
    );
    match work {
        Some(w) => {
            *result = w;
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_pool_has_opencl(handle: *mut WorkPoolHandle) -> bool {
    (*handle).0.has_opencl()
}
