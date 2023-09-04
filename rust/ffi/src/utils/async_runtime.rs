use std::{ffi::c_void, sync::Arc};

use rsnano_node::utils::AsyncRuntime;

pub struct AsyncRuntimeHandle(Arc<AsyncRuntime>);

#[no_mangle]
pub extern "C" fn rsn_async_runtime_create(io_ctx: *mut c_void) -> *mut AsyncRuntimeHandle {
    Box::into_raw(Box::new(AsyncRuntimeHandle(Arc::new(AsyncRuntime {
        cpp: io_ctx,
        tokio: Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .thread_name("tokio runtime")
                .enable_all()
                .build()
                .unwrap(),
        ),
    }))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_async_runtime_destroy(handle: *mut AsyncRuntimeHandle) {
    drop(Box::from_raw(handle));
}
