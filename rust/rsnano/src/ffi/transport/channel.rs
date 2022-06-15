use std::{
    ops::Deref,
    sync::{Arc, MutexGuard},
};

use crate::transport::{Channel, ChannelData, ChannelTcp, ChannelInProc, ChannelUdp};

use super::socket::SocketHandle;

enum ChannelType {
    Tcp(ChannelTcp),
    InProc(ChannelInProc),
    Udp(ChannelUdp),
}

pub struct ChannelHandle(Arc<ChannelType>);

unsafe fn as_tcp_channel(handle: *mut ChannelHandle) -> &'static ChannelTcp {
    match (*handle).0.as_ref() {
        ChannelType::Tcp(tcp) => tcp,
        _ => panic!("expected tcp channel"),
    }
}

unsafe fn as_channel(handle: *mut ChannelHandle) -> &'static dyn Channel {
    match (*handle).0.as_ref() {
        ChannelType::Tcp(tcp) => tcp,
        ChannelType::InProc(inproc) => inproc,
        ChannelType::Udp(udp) => udp,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_destroy(handle: *mut ChannelHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_create(socket: *mut SocketHandle) -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelType::Tcp(
        ChannelTcp::new((*socket).deref()),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_is_temporary(handle: *mut ChannelHandle) -> bool {
    as_channel(handle).is_temporary()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_set_temporary(handle: *mut ChannelHandle, temporary: bool) {
    as_channel(handle).set_temporary(temporary);
}

pub struct TcpChannelLockHandle(MutexGuard<'static, ChannelData>);

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_lock(
    handle: *mut ChannelHandle,
) -> *mut TcpChannelLockHandle {
    let tcp = as_tcp_channel(handle);
    Box::into_raw(Box::new(TcpChannelLockHandle(std::mem::transmute::<
        MutexGuard<ChannelData>,
        MutexGuard<'static, ChannelData>,
    >(tcp.lock()))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_unlock(handle: *mut TcpChannelLockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_channel_inproc_create() -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelType::InProc(ChannelInProc::new())))))
}

#[no_mangle]
pub extern "C" fn rsn_channel_udp_create() -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelType::Udp(ChannelUdp::new())))))
}
