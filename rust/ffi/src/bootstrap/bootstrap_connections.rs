use super::{bootstrap_client::BootstrapClientHandle, pulls_cache::PullInfoDto};
use crate::{transport::EndpointDto, VoidPointerCallback};
use rsnano_node::bootstrap::{
    BootstrapConnections, ADD_PULL_CALLBACK, CONNECTION_CALLBACK,
    DROP_BOOTSTRAP_CONNECTIONS_CALLBACK, FIND_CONNECTION_CALLBACK, POOL_CONNECTION_CALLBACK,
    POPULATE_CONNECTIONS_CALLBACK, REQUEUE_PULL_CALLBACK,
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

pub type BootstrapConnectionsConnectionCallback =
    unsafe extern "C" fn(*mut c_void, bool, *mut bool) -> *mut BootstrapClientHandle;

pub type BootstrapConnectionsFindConnectionCallback =
    unsafe extern "C" fn(*mut c_void, *const EndpointDto) -> *mut BootstrapClientHandle;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_connections_dropped(callback: VoidPointerCallback) {
    unsafe {
        DROP_BOOTSTRAP_CONNECTIONS_CALLBACK = Some(callback);
    }
}

static mut FFI_POOL_CONNECTION_CALLBACK: Option<PoolConnectionCallback> = None;
static mut FFI_REQUEUE_PULL_CALLBACK: Option<RequeuePullCallback> = None;
static mut FFI_ADD_PULL_CALLBACK: Option<AddPullCallback> = None;
static mut FFI_CONNECTIONS_CALLBACK: Option<BootstrapConnectionsConnectionCallback> = None;
static mut FFI_FIND_CONNECTION_CALLBACK: Option<BootstrapConnectionsFindConnectionCallback> = None;

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

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_connections_connection(
    callback: BootstrapConnectionsConnectionCallback,
) {
    unsafe {
        FFI_CONNECTIONS_CALLBACK = Some(callback);
        CONNECTION_CALLBACK = Some(|cpp_handle, use_front_connection| {
            let mut should_stop = true;
            let handle = FFI_CONNECTIONS_CALLBACK.unwrap()(
                cpp_handle,
                use_front_connection,
                &mut should_stop,
            );

            let client = if handle.is_null() {
                None
            } else {
                let client = Some(Arc::clone(&**handle));
                drop(Box::from_raw(handle));
                client
            };

            (client, should_stop)
        });
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_bootstrap_connections_find_connection(
    callback: BootstrapConnectionsFindConnectionCallback,
) {
    unsafe {
        FFI_FIND_CONNECTION_CALLBACK = Some(callback);
        FIND_CONNECTION_CALLBACK = Some(|cpp_handle, endpoint| {
            let endpoint_dto = EndpointDto::from(endpoint);
            let client_handle = FFI_FIND_CONNECTION_CALLBACK.unwrap()(cpp_handle, &endpoint_dto);
            if client_handle.is_null() {
                None
            } else {
                let client = Some(Arc::clone(&**client_handle));
                drop(Box::from_raw(client_handle));
                client
            }
        });
    }
}
