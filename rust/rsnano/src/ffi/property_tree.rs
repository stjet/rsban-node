use crate::{PropertyTreeReader, PropertyTreeWriter};
use anyhow::Result;
use std::{
    ffi::{c_void, CString},
    os::raw::c_char,
};

type PropertyTreePutStringCallback =
    unsafe extern "C" fn(*mut c_void, *const c_char, usize, *const c_char, usize);
type PropertyTreeGetStringCallback =
    unsafe extern "C" fn(*const c_void, *const c_char, usize, *mut c_char, usize) -> i32;

type PropertyTreeCreateTreeCallback = unsafe extern "C" fn() -> *mut c_void;
type PropertyTreeDestroyTreeCallback = unsafe extern "C" fn(*mut c_void);
type PropertyTreePushBackCallback = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_void);

static mut PUT_STRING_CALLBACK: Option<PropertyTreePutStringCallback> = None;
static mut ADD_CALLBACK: Option<PropertyTreePutStringCallback> = None;
static mut GET_STRING_CALLBACK: Option<PropertyTreeGetStringCallback> = None;
static mut CREATE_TREE_CALLBACK: Option<PropertyTreeCreateTreeCallback> = None;
static mut DESTROY_TREE_CALLBACK: Option<PropertyTreeDestroyTreeCallback> = None;
static mut PUSH_BACK_CALLBACK: Option<PropertyTreePushBackCallback> = None;
static mut ADD_CHILD_CALLBACK: Option<PropertyTreePushBackCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_property_tree_put_string(f: PropertyTreePutStringCallback) {
    PUT_STRING_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_property_tree_add(f: PropertyTreePutStringCallback) {
    ADD_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_property_tree_get_string(f: PropertyTreeGetStringCallback) {
    GET_STRING_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_property_tree_create(f: PropertyTreeCreateTreeCallback) {
    CREATE_TREE_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_property_tree_destroy(f: PropertyTreeDestroyTreeCallback) {
    DESTROY_TREE_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_property_tree_push_back(f: PropertyTreePushBackCallback) {
    PUSH_BACK_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_property_tree_add_child(f: PropertyTreePushBackCallback) {
    ADD_CHILD_CALLBACK = Some(f);
}

pub struct FfiPropertyTreeWriter {
    handle: *mut c_void,
    owned: bool,
}

impl FfiPropertyTreeWriter {
    /// don't free the handle
    pub fn new_borrowed(handle: *mut c_void) -> Self {
        Self {
            handle,
            owned: false,
        }
    }

    /// free the handle when dropped
    pub fn new_owned(handle: *mut c_void) -> Self {
        Self {
            handle,
            owned: true,
        }
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

    fn new_writer(&self) -> Box<dyn PropertyTreeWriter> {
        let handle = unsafe {
            match CREATE_TREE_CALLBACK {
                Some(f) => f(),
                None => panic!("CREATE_TREE_CALLBACK missing"),
            }
        };
        Box::new(FfiPropertyTreeWriter::new_owned(handle))
    }

    fn push_back(&mut self, path: &str, value: &dyn PropertyTreeWriter) {
        unsafe {
            match PUSH_BACK_CALLBACK {
                Some(f) => {
                    let path_str = CString::new(path).unwrap();
                    let ffi_value = value
                        .as_any()
                        .downcast_ref::<FfiPropertyTreeWriter>()
                        .unwrap();
                    f(self.handle, path_str.as_ptr(), ffi_value.handle);
                }
                None => panic!("PUSH_BACK_CALLBACK missing"),
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn add_child(&mut self, path: &str, value: &dyn PropertyTreeWriter) {
        unsafe {
            match ADD_CHILD_CALLBACK {
                Some(f) => {
                    let path_str = CString::new(path).unwrap();
                    let ffi_value = value
                        .as_any()
                        .downcast_ref::<FfiPropertyTreeWriter>()
                        .unwrap();
                    f(self.handle, path_str.as_ptr(), ffi_value.handle);
                }
                None => panic!("ADD_CHILD_CALLBACK missing"),
            }
        }
    }

    fn add(&mut self, path: &str, value: &str) -> Result<()> {
        unsafe {
            match ADD_CALLBACK {
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
                None => Err(anyhow!("ADD_CALLBACK missing")),
            }
        }
    }
}

impl Drop for FfiPropertyTreeWriter {
    fn drop(&mut self) {
        if self.owned {
            unsafe {
                match DESTROY_TREE_CALLBACK {
                    Some(f) => f(self.handle),
                    None => panic!("DESTROY_TREE_CALLBACK missing"),
                }
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
