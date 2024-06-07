use super::BootstrapInitiatorHandle;
use crate::{
    block_processing::BlockProcessorHandle,
    ledger::datastore::LedgerHandle,
    transport::{
        EndpointDto, SocketFfiObserver, SocketHandle, SynCookiesHandle, TcpChannelsHandle,
    },
    utils::{AsyncRuntimeHandle, ContextWrapper, ThreadPoolHandle},
    ErrorCodeDto, NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle, TcpConfigDto,
    VoidPointerCallback,
};
use rsnano_core::KeyPair;
use rsnano_node::{
    transport::{Socket, TcpListener, TcpListenerExt},
    utils::ErrorCode,
};
use std::{ffi::c_void, ops::Deref, sync::Arc};
use tracing::debug;

pub struct TcpListenerHandle(pub Arc<TcpListener>);

impl Deref for TcpListenerHandle {
    type Target = Arc<TcpListener>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tcp_listener_destroy(handle: *mut TcpListenerHandle) {
    debug!("calling TCP listener destroy");
    drop(Box::from_raw(handle))
}

pub type OnConnectionCallback =
    extern "C" fn(*mut c_void, *mut SocketHandle, *const ErrorCodeDto) -> bool;

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_realtime_count(handle: &TcpListenerHandle) -> usize {
    handle.0.realtime_count()
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_connection_count(handle: &TcpListenerHandle) -> usize {
    handle.0.connection_count()
}

#[no_mangle]
pub extern "C" fn rsn_tcp_listener_endpoint(handle: &TcpListenerHandle, result: &mut EndpointDto) {
    *result = handle.0.local_address().into()
}
