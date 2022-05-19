use crate::UncheckedInfo;

use super::BlockHandle;

pub struct UncheckedInfoHandle(UncheckedInfo);

#[no_mangle]
pub extern "C" fn rsn_unchecked_info_create() -> *mut UncheckedInfoHandle {
    let info = UncheckedInfo::null();
    Box::into_raw(Box::new(UncheckedInfoHandle(info)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_info_create2(
    block: *const BlockHandle,
) -> *mut UncheckedInfoHandle {
    let block = (*block).block.clone();
    let info = UncheckedInfo::new(block);
    Box::into_raw(Box::new(UncheckedInfoHandle(info)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_info_clone(
    handle: *const UncheckedInfoHandle,
) -> *mut UncheckedInfoHandle {
    Box::into_raw(Box::new(UncheckedInfoHandle((*handle).0.clone())))
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
pub unsafe extern "C" fn rsn_unchecked_info_block_set(
    handle: *mut UncheckedInfoHandle,
    block: *mut BlockHandle,
) {
    (*handle).0.block = Some((*block).block.clone());
}
