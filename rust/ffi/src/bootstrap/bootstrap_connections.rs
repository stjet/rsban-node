use super::{bootstrap_client::BootstrapClientHandle, pulls_cache::PullInfoDto};
use crate::VoidPointerCallback;
use rsnano_node::bootstrap::{
    BootstrapConnections, ADD_PULL_CALLBACK, DROP_BOOTSTRAP_CONNECTIONS_CALLBACK,
    POOL_CONNECTION_CALLBACK, POPULATE_CONNECTIONS_CALLBACK, REQUEUE_PULL_CALLBACK,
};
use std::{ffi::c_void, ops::Deref, sync::Arc};

pub struct BootstrapConnectionsHandle(Arc<BootstrapConnections>);

impl Deref for BootstrapConnectionsHandle {
    type Target = Arc<BootstrapConnections>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_connections_create(
    cpp_handle: *mut c_void,
) -> *mut BootstrapConnectionsHandle {
    Box::into_raw(Box::new(BootstrapConnectionsHandle(Arc::new(
        BootstrapConnections::new(cpp_handle),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_connections_drop(handle: *mut BootstrapConnectionsHandle) {
    drop(Box::from_raw(handle));
}

pub type PoolConnectionCallback =
    unsafe extern "C" fn(*mut c_void, *mut BootstrapClientHandle, bool, bool);

pub type RequeuePullCallback = unsafe extern "C" fn(*mut c_void, *const PullInfoDto, bool);

pub type PopulateConnectionsCallback = unsafe extern "C" fn(*mut c_void, bool);

pub type AddPullCallback = unsafe extern "C" fn(*mut c_void, *const PullInfoDto);

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_connections_dropped(callback: VoidPointerCallback) {
    unsafe {
        DROP_BOOTSTRAP_CONNECTIONS_CALLBACK = Some(callback);
    }
}

static mut FFI_POOL_CONNECTION_CALLBACK: Option<PoolConnectionCallback> = None;
static mut FFI_REQUEUE_PULL_CALLBACK: Option<RequeuePullCallback> = None;
static mut FFI_ADD_PULL_CALLBACK: Option<AddPullCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_connections_pool_connection(
    callback: PoolConnectionCallback,
) {
    unsafe {
        FFI_POOL_CONNECTION_CALLBACK = Some(callback);
        POOL_CONNECTION_CALLBACK = Some(|cpp_handle, client, new_client, push_front| {
            let client_handle = Box::into_raw(Box::new(BootstrapClientHandle(client)));
            FFI_POOL_CONNECTION_CALLBACK.unwrap()(
                cpp_handle,
                client_handle,
                new_client,
                push_front,
            );
        });
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_connections_requeue_pull(
    callback: RequeuePullCallback,
) {
    unsafe {
        FFI_REQUEUE_PULL_CALLBACK = Some(callback);
        REQUEUE_PULL_CALLBACK = Some(|cpp_handle, pull, network_error| {
            let pull_dto = (&pull).into();
            FFI_REQUEUE_PULL_CALLBACK.unwrap()(cpp_handle, &pull_dto, network_error);
        });
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_connections_populate_connections(
    callback: PopulateConnectionsCallback,
) {
    unsafe {
        POPULATE_CONNECTIONS_CALLBACK = Some(callback);
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_connections_add_pull(callback: AddPullCallback) {
    unsafe {
        FFI_ADD_PULL_CALLBACK = Some(callback);
        ADD_PULL_CALLBACK = Some(|cpp_handle, pull| {
            let pull_dto = (&pull).into();
            FFI_ADD_PULL_CALLBACK.unwrap()(cpp_handle, &pull_dto);
        });
    }
}
