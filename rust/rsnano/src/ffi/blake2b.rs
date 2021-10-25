use std::ffi::c_void;

type Blake2BInitCallback = unsafe extern "C" fn(*mut c_void, usize) -> i32;
type Blake2BUpdateCallback = unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> i32;
type Blake2BFinalCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> i32;

static mut INIT_CALLBACK: Option<Blake2BInitCallback> = None;
static mut UPDATE_CALLBACK: Option<Blake2BUpdateCallback> = None;
static mut FINAL_CALLBACK: Option<Blake2BFinalCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_blake2b_init(f: Blake2BInitCallback) {
    INIT_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_blake2b_update(f: Blake2BUpdateCallback) {
    UPDATE_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_blake2b_final(f: Blake2BFinalCallback) {
    FINAL_CALLBACK = Some(f);
}