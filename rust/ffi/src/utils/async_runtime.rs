use std::{ffi::c_void, sync::Arc};

use rsnano_node::utils::AsyncRuntime;

use super::ContextWrapper;
use crate::VoidPointerCallback;

pub struct AsyncRuntimeHandle(pub Arc<AsyncRuntime>);

#[no_mangle]
pub extern "C" fn rsn_async_runtime_create(_multi_threaded: bool) -> *mut AsyncRuntimeHandle {
    let multi_threaded = true;
    //todo! use single threaded runtime for tests
    if multi_threaded {
        Box::into_raw(Box::new(AsyncRuntimeHandle(Arc::new(AsyncRuntime::new(
            tokio::runtime::Builder::new_multi_thread()
                .thread_name("tokio runtime")
                .enable_all()
                .build()
                .unwrap(),
        )))))
    } else {
        Box::into_raw(Box::new(AsyncRuntimeHandle(Arc::new(AsyncRuntime::new(
            tokio::runtime::Builder::new_current_thread()
                .thread_name("tokio runtime")
                .enable_all()
                .build()
                .unwrap(),
        )))))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_runtime_destroy(handle: *mut AsyncRuntimeHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_runtime_post(
    handle: &AsyncRuntimeHandle,
    callback: VoidPointerCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(context, delete_context);
    let callback_wrapper = Box::new(move || {
        callback(context_wrapper.get_context());
    });
    handle.0.tokio.spawn_blocking(callback_wrapper);
}
