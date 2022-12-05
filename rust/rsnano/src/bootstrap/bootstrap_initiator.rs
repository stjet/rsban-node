use std::ffi::c_void;

pub type BootstrapInitiatorClearPullsCallback = unsafe extern "C" fn(*mut c_void, u64);
pub static mut BOOTSTRAP_INITIATOR_CLEAR_PULLS_CALLBACK: Option<
    BootstrapInitiatorClearPullsCallback,
> = None;

pub struct BootstrapInitiator {
    handle: *mut c_void,
}

impl BootstrapInitiator {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }

    pub fn clear_pulls(&self, bootstrap_id: u64) {
        unsafe {
            match BOOTSTRAP_INITIATOR_CLEAR_PULLS_CALLBACK {
                Some(f) => f(self.handle, bootstrap_id),
                None => panic!("BOOTSTRAP_INITIATOR_CLEAR_PULLS_CALLBACK missing"),
            }
        }
    }
}
