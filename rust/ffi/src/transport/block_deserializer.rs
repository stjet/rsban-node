use crate::{core::BlockHandle, utils::ContextWrapper, ErrorCodeDto, VoidPointerCallback};
use rsnano_node::transport::BlockDeserializer;
use std::{ffi::c_void, sync::Arc};

use super::SocketHandle;

pub struct BlockDeserializerHandle(BlockDeserializer);

#[no_mangle]
pub extern "C" fn rsn_block_deserializer_create() -> *mut BlockDeserializerHandle {
    Box::into_raw(Box::new(BlockDeserializerHandle(BlockDeserializer::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_deserializer_destroy(handle: *mut BlockDeserializerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_block_deserializer_read(
    handle: &BlockDeserializerHandle,
    socket: &SocketHandle,
    callback: BlockDeserializedCallback,
    context: *mut c_void,
    drop_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(context, drop_context);
    handle.0.read(
        socket,
        Box::new(move |ec, block| {
            let block_handle = match block {
                Some(b) => BlockHandle::new(Arc::new(b)),
                None => std::ptr::null_mut(),
            };
            callback(context_wrapper.get_context(), &(&ec).into(), block_handle);
        }),
    )
}

pub type BlockDeserializedCallback =
    extern "C" fn(*mut c_void, *const ErrorCodeDto, *mut BlockHandle);
