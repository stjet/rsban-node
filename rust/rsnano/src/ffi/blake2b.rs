use std::ffi::c_void;

type Blake2BInitCallback = unsafe extern "C" fn(*mut c_void, usize) -> i32;
type Blake2BUpdateCallback = unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> i32;
type Blake2BFinalCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> i32;

