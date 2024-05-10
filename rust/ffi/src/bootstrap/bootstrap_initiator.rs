use rsnano_node::bootstrap::{
    BootstrapInitiator, BOOTSTRAP_INITIATOR_BOOTSTRAP_LAZY,
    BOOTSTRAP_INITIATOR_CLEAR_PULLS_CALLBACK, BOOTSTRAP_INITIATOR_IN_PROGRESS_CALLBACK,
    BOOTSTRAP_INITIATOR_REMOVE_CACHE_CALLBACK,
};
use std::{
    ffi::{c_char, c_void},
    ops::Deref,
    sync::Arc,
};

use super::pulls_cache::PullInfoDto;

pub struct BootstrapInitiatorHandle(Arc<BootstrapInitiator>);

impl Deref for BootstrapInitiatorHandle {
    type Target = Arc<BootstrapInitiator>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_create(
    handle: *mut c_void,
) -> *mut BootstrapInitiatorHandle {
    Box::into_raw(Box::new(BootstrapInitiatorHandle(Arc::new(
        BootstrapInitiator::new(handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_initiator_destroy(handle: *mut BootstrapInitiatorHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_initiator_clear_pulls(
    f: BootstrapInitiatorClearPullsCallback,
) {
    BOOTSTRAP_INITIATOR_CLEAR_PULLS_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_initiator_in_progress(
    f: BootstrapInitiatorInProgressCallback,
) {
    BOOTSTRAP_INITIATOR_IN_PROGRESS_CALLBACK = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_initiator_remove_from_cache(
    f: BootstrapInitiatorRemoveCacheCallback,
) {
    FFI_BOOTSTRAP_INITIATOR_REMOVE_CACHE_CALLBACK = Some(f);
    BOOTSTRAP_INITIATOR_REMOVE_CACHE_CALLBACK = Some(|handle, pull| {
        let pull_dto = PullInfoDto::from(pull);
        unsafe { FFI_BOOTSTRAP_INITIATOR_REMOVE_CACHE_CALLBACK.unwrap()(handle, &pull_dto) };
    });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_initiator_bootstrap_lazy(
    f: BootstrapInitiatorBootstrapLazyCallback,
) {
    FFI_BOOTSTRAP_INITIATOR_BOOTSTRAP_LAZY = Some(f);
    BOOTSTRAP_INITIATOR_BOOTSTRAP_LAZY = Some(|handle, account, force, id| unsafe {
        FFI_BOOTSTRAP_INITIATOR_BOOTSTRAP_LAZY.unwrap()(
            handle,
            account.as_bytes().as_ptr(),
            force,
            id.as_ptr() as *const c_char,
            id.len(),
        )
    });
}

pub type BootstrapInitiatorClearPullsCallback = unsafe extern "C" fn(*mut c_void, u64);
pub type BootstrapInitiatorInProgressCallback = unsafe extern "C" fn(*mut c_void) -> bool;
pub type BootstrapInitiatorRemoveCacheCallback =
    unsafe extern "C" fn(*mut c_void, *const PullInfoDto);
pub type BootstrapInitiatorBootstrapLazyCallback =
    unsafe extern "C" fn(*mut c_void, *const u8, bool, *const c_char, usize) -> bool;

static mut FFI_BOOTSTRAP_INITIATOR_REMOVE_CACHE_CALLBACK: Option<
    BootstrapInitiatorRemoveCacheCallback,
> = None;

static mut FFI_BOOTSTRAP_INITIATOR_BOOTSTRAP_LAZY: Option<BootstrapInitiatorBootstrapLazyCallback> =
    None;
