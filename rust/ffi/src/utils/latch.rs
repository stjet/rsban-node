use rsnano_core::utils::Latch;
use std::ffi::c_void;

type WaitLatchCallback = unsafe extern "C" fn(*mut c_void);

static mut WAIT_LATCH: Option<WaitLatchCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_set_wait_latch_callback(f: WaitLatchCallback) {
    WAIT_LATCH = Some(f);
}

pub struct FfiLatch {
    latch_ptr: *mut c_void,
}

unsafe impl Send for FfiLatch {}
unsafe impl Sync for FfiLatch {}

impl Latch for FfiLatch {
    fn wait(&self) {
        unsafe {
            WAIT_LATCH.expect("WAIT_LATCH callback missing")(self.latch_ptr);
        }
    }
}
