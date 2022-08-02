use std::{ffi::c_void, sync::Arc};

use crate::{
    bootstrap::{BootstrapClient, BootstrapClientObserver, BootstrapClientObserverWeakPtr},
    ffi::DestroyCallback,
};

pub struct BootstrapClientHandle(BootstrapClient);

/// `observer` is a `shared_ptr<bootstrap_client_observer>*`
#[no_mangle]
pub extern "C" fn rsn_bootstrap_client_create(observer: *mut c_void) -> *mut BootstrapClientHandle {
    let observer = Arc::new(FfiBootstrapClientObserver::new(observer));
    Box::into_raw(Box::new(BootstrapClientHandle(BootstrapClient::new(
        observer,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_destroy(handle: *mut BootstrapClientHandle) {
    drop(Box::from_raw(handle))
}

struct FfiBootstrapClientObserver {
    /// a `shared_ptr<bootstrap_client_observer>*`
    handle: *mut c_void,
}

impl FfiBootstrapClientObserver {
    fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

impl BootstrapClientObserver for FfiBootstrapClientObserver {
    fn bootstrap_client_closed(&self) {
        unsafe {
            CLIENT_CLOSED.expect("CLIENT_CLOSED missing")(self.handle);
        }
    }

    fn to_weak(&self) -> Box<dyn BootstrapClientObserverWeakPtr> {
        let weak_handle = unsafe {
            OBSERVER_TO_WEAK.expect("OBSERVER_TO_WEAK missing")(self.handle)
        };
        Box::new(FfiBootstrapClientObserverWeakPtr::new(weak_handle))
    }
}

impl Drop for FfiBootstrapClientObserver {
    fn drop(&mut self) {
        unsafe { DROP_OBSERVER.expect("DROP_OBSERVER missing")(self.handle) }
    }
}

struct FfiBootstrapClientObserverWeakPtr {
    /// a `weak_ptr<bootstrap_client_observer>*`
    handle: *mut c_void,
}

impl FfiBootstrapClientObserverWeakPtr {
    fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

impl BootstrapClientObserverWeakPtr for FfiBootstrapClientObserverWeakPtr {
    fn upgrade(&self) -> Option<Arc<dyn BootstrapClientObserver>> {
        let observer_handle =
            unsafe { OBSERVER_TO_WEAK.expect("OBSERVER_TO_WEAK missing")(self.handle) };
        if observer_handle.is_null() {
            None
        } else {
            Some(Arc::new(FfiBootstrapClientObserver::new(observer_handle)))
        }
    }
}

impl Drop for FfiBootstrapClientObserverWeakPtr {
    fn drop(&mut self) {
        unsafe { DROP_WEAK.expect("DROP_WEAK missing")(self.handle) }
    }
}

pub type BootstrapClientClosedCallback = unsafe extern "C" fn(*mut c_void);

static mut CLIENT_CLOSED: Option<BootstrapClientClosedCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_client_observer_closed(
    f: BootstrapClientClosedCallback,
) {
    CLIENT_CLOSED = Some(f);
}

/// takes a `shared_ptr<bootstrap_client_observer>*` and
pub type BootstrapClientObserverToWeakCallback = unsafe extern "C" fn(*mut c_void) -> *mut c_void;

static mut OBSERVER_TO_WEAK: Option<BootstrapClientObserverToWeakCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_client_observer_to_weak(
    f: BootstrapClientObserverToWeakCallback,
) {
    OBSERVER_TO_WEAK = Some(f);
}

static mut DROP_WEAK: Option<DestroyCallback> = None;
static mut DROP_OBSERVER: Option<DestroyCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_client_observer_destroy(f: DestroyCallback) {
    DROP_OBSERVER = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_client_observer_weak_destroy(f: DestroyCallback) {
    DROP_WEAK = Some(f);
}
