use rsnano_ledger::WriteGuard;

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
