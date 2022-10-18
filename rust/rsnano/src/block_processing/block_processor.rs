use std::ffi::c_void;

use crate::{
    ffi::{UncheckedInfoHandle, BLOCKPROCESSOR_ADD_CALLBACK}, core::UncheckedInfo,
};

pub struct BlockProcessor {
    handle: *mut c_void,
}

impl BlockProcessor {
    pub(crate) fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }

    pub(crate) fn add(&self, info: &UncheckedInfo) {
        unsafe {
            match BLOCKPROCESSOR_ADD_CALLBACK {
                Some(f) => f(
                    self.handle,
                    Box::into_raw(Box::new(UncheckedInfoHandle::new(info.clone()))),
                ),
                None => panic!("BLOCKPROCESSOR_ADD_CALLBACK missing"),
            }
        }
    }
}
