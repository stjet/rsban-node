use std::sync::{Arc, MutexGuard};

use crate::transport::{ChannelData, ChannelTcp};

enum ChannelType {
    Tcp(ChannelTcp),
    InProc,
    Udp,
}

pub struct ChannelHandle(Arc<ChannelType>);

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_destroy(handle: *mut ChannelHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub extern "C" fn rsn_channel_tcp_create() -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelType::Tcp(
        ChannelTcp::new(),
    )))))
}

unsafe fn as_tcp_channel(handle: *mut ChannelHandle) -> &'static ChannelTcp {
    match (*handle).0.as_ref() {
        ChannelType::Tcp(tcp) => tcp,
        _ => panic!("expected tcp channel"),
    }
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
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelType::InProc))))
}

#[no_mangle]
pub extern "C" fn rsn_channel_udp_create() -> *mut ChannelHandle {
    Box::into_raw(Box::new(ChannelHandle(Arc::new(ChannelType::Udp))))
}
