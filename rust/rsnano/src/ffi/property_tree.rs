use anyhow::Result;
use std::{ffi::c_void, os::raw::c_char};

use crate::utils::{PropertyTreeReader, PropertyTreeWriter};

type PropertyTreePutStringCallback =
    unsafe extern "C" fn(*mut c_void, *const c_char, usize, *const c_char, usize);
type PropertyTreeGetStringCallback =
    unsafe extern "C" fn(*const c_void, *const c_char, usize, *mut c_char, usize) -> i32;

static mut PUT_STRING_CALLBACK: Option<PropertyTreePutStringCallback> = None;
static mut GET_STRING_CALLBACK: Option<PropertyTreeGetStringCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_property_tree_put_string(f: PropertyTreePutStringCallback) {
    PUT_STRING_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_property_tree_get_string(f: PropertyTreeGetStringCallback) {
    GET_STRING_CALLBACK = Some(f);
}

pub struct FfiPropertyTreeWriter {
    handle: *mut c_void,
}

impl FfiPropertyTreeWriter {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

impl PropertyTreeWriter for FfiPropertyTreeWriter {
    fn put_string(&mut self, path: &str, value: &str) -> Result<()> {
        unsafe {
            match PUT_STRING_CALLBACK {
                Some(f) => {
                    f(
                        self.handle,
                        path.as_ptr() as *const i8,
                        path.len(),
                        value.as_ptr() as *const i8,
                        value.len(),
                    );
                    Ok(())
                }
                None => Err(anyhow!("PUT_STRING_CALLBACK missing")),
            }
        }
    }
}

const PROPERTY_TREE_BUFFER_SIZE: usize = 1024;

pub struct FfiPropertyTreeReader {
    handle: *const c_void,
}

impl FfiPropertyTreeReader {
    pub fn new(handle: *const c_void) -> Self {
        Self { handle }
    }
}

impl PropertyTreeReader for FfiPropertyTreeReader {
    fn get_string(&self, path: &str) -> Result<String> {
        unsafe {
            match GET_STRING_CALLBACK {
                Some(f) => {
                    let mut buffer = [0u8; PROPERTY_TREE_BUFFER_SIZE];
                    let read_count = f(
                        self.handle,
                        path.as_ptr() as *const i8,
                        path.len(),
                        buffer.as_mut_ptr() as *mut i8,
                        PROPERTY_TREE_BUFFER_SIZE,
                    );
                    if read_count < 0 {
                        bail!("GET_STRING_CALLBACK failed");
                    }
                    Ok(String::from_utf8_lossy(&buffer[..read_count as usize]).into_owned())
                }
                None => Err(anyhow!("GET_STRING_CALLBACK missing")),
            }
        }
    }
}
