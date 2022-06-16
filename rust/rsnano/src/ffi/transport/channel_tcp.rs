use super::{
    channel::{as_tcp_channel, ChannelHandle, ChannelType},
    socket::SocketHandle,
};
use crate::transport::{ChannelTcp, TcpChannelData};
use std::{
    ops::Deref,
    sync::{Arc, MutexGuard},
};

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_create(
    now: u64,
    socket: *mut SocketHandle,
) -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle::new(Arc::new(ChannelType::Tcp(
        ChannelTcp::new((*socket).deref(), now),
    )))))
}

pub struct TcpChannelLockHandle(MutexGuard<'static, TcpChannelData>);

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_lock(
    handle: *mut ChannelHandle,
) -> *mut TcpChannelLockHandle {
    let tcp = as_tcp_channel(handle);
    Box::into_raw(Box::new(TcpChannelLockHandle(std::mem::transmute::<
        MutexGuard<TcpChannelData>,
        MutexGuard<'static, TcpChannelData>,
    >(tcp.lock()))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_unlock(handle: *mut TcpChannelLockHandle) {
    drop(Box::from_raw(handle))
}
