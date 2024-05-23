use rsnano_node::utils::AsyncRuntime;
use std::{ops::Deref, sync::Arc};

pub struct AsyncRuntimeHandle(pub Arc<AsyncRuntime>);

impl Deref for AsyncRuntimeHandle {
    type Target = Arc<AsyncRuntime>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
