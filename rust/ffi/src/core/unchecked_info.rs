use rsnano_core::utils::{Deserialize, Serialize};
use rsnano_core::UncheckedInfo;
use std::ffi::c_void;
use std::ops::Deref;

use crate::{core::BlockHandle, utils::FfiStream};

pub struct UncheckedInfoHandle(pub UncheckedInfo);

impl UncheckedInfoHandle {
    pub fn new(info: UncheckedInfo) -> Self {
        Self(info)
    }
}

impl Deref for UncheckedInfoHandle {
    type Target = UncheckedInfo;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_unchecked_info_create() -> *mut UncheckedInfoHandle {
    let info = UncheckedInfo::null();
    Box::into_raw(Box::new(UncheckedInfoHandle::new(info)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_info_create2(
    block: *const BlockHandle,
) -> *mut UncheckedInfoHandle {
    let block = (*block).block.clone();
    let info = UncheckedInfo::new(block);
    Box::into_raw(Box::new(UncheckedInfoHandle::new(info)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_info_clone(
    handle: *const UncheckedInfoHandle,
) -> *mut UncheckedInfoHandle {
    Box::into_raw(Box::new(UncheckedInfoHandle::new((*handle).0.clone())))
}

#[no_mangle]
pub extern "C" fn rsn_unchecked_info_destroy(handle: *mut UncheckedInfoHandle) {
    drop(unsafe { Box::from_raw(handle) });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_info_block(
    handle: *const UncheckedInfoHandle,
) -> *mut BlockHandle {
    Box::into_raw(Box::new(BlockHandle {
        block: (*handle).0.block.as_ref().unwrap().clone(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_info_modified(handle: *const UncheckedInfoHandle) -> u64 {
    (*handle).0.modified
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_info_serialize(
    handle: *mut UncheckedInfoHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    (*handle).0.serialize(&mut stream).is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_info_deserialize(
    handle: *mut UncheckedInfoHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    match UncheckedInfo::deserialize(&mut stream) {
        Ok(info) => {
            (*handle).0 = info;
            true
        }
        Err(_) => false,
    }
}
