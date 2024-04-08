use num_traits::FromPrimitive;
use rsnano_ledger::{WriteGuard, WriteQueue, Writer};
use std::{ops::Deref, sync::Arc};

pub struct WriteQueueHandle(pub Arc<WriteQueue>);

impl Deref for WriteQueueHandle {
    type Target = Arc<WriteQueue>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_write_queue_create(use_noop: bool) -> *mut WriteQueueHandle {
    Box::into_raw(Box::new(WriteQueueHandle(Arc::new(WriteQueue::new(
        use_noop,
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_write_queue_destroy(handle: *mut WriteQueueHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_write_queue_wait(
    handle: *mut WriteQueueHandle,
    writer: u8,
) -> *mut WriteGuardHandle {
    let guard = (*handle).0.wait(Writer::from_u8(writer).unwrap());
    WriteGuardHandle::new(guard)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_write_queue_contains(
    handle: *mut WriteQueueHandle,
    writer: u8,
) -> bool {
    (*handle).0.contains(Writer::from_u8(writer).unwrap())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_write_queue_process(
    handle: *mut WriteQueueHandle,
    writer: u8,
) -> bool {
    (*handle).0.process(Writer::from_u8(writer).unwrap())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_write_queue_pop(
    handle: *mut WriteQueueHandle,
) -> *mut WriteGuardHandle {
    WriteGuardHandle::new((*handle).0.pop())
}

pub struct WriteGuardHandle(pub WriteGuard);
impl WriteGuardHandle {
    pub fn new(guard: WriteGuard) -> *mut WriteGuardHandle {
        Box::into_raw(Box::new(WriteGuardHandle(guard)))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_write_guard_release(handle: *mut WriteGuardHandle) {
    (*handle).0.release();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_write_guard_destroy(handle: *mut WriteGuardHandle) {
    drop(Box::from_raw(handle))
}
