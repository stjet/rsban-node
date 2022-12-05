use std::{ffi::c_void, net::SocketAddr, sync::Arc};

use rsnano_node::{
    messages::Message,
    transport::{ChannelTcpObserver, IChannelTcpObserverWeakPtr},
};

use crate::{messages::MessageHandle, VoidPointerCallback};

use super::socket::EndpointDto;

pub static mut DROP_WEAK_PTR: Option<VoidPointerCallback> = None;

/// input is a `weak_ptr<channel_tcp_observer> *`
/// output is a `shared_ptr<channel_tcp_observer> *` or `nullptr`
pub type ChannelTcpObserverLockWeakCallback = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
pub static mut LOCK_OBSERVER: Option<ChannelTcpObserverLockWeakCallback> = None;

/// clones a `weak_ptr<channel_tcp_observer> *`
pub type ChannelTcpObserverWeakCloneCallback = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
pub static mut CLONE_WEAK_PTR: Option<ChannelTcpObserverWeakCloneCallback> = None;

pub type ChannelTcpObserverDataSentCallback = unsafe extern "C" fn(*mut c_void, *const EndpointDto);
pub static mut DATA_SENT: Option<ChannelTcpObserverDataSentCallback> = None;

pub type ChannelTcpObserverCallback = unsafe extern "C" fn(*mut c_void);
pub static mut HOST_UNREACHABLE: Option<ChannelTcpObserverCallback> = None;

pub type ChannelTcpObserverMessageSentCallback =
    unsafe extern "C" fn(*mut c_void, message: *mut MessageHandle);
pub static mut MESSAGE_SENT: Option<ChannelTcpObserverMessageSentCallback> = None;

pub type ChannelTcpObserverMessageDroppedCallback =
    unsafe extern "C" fn(*mut c_void, message: *mut MessageHandle, usize);
pub static mut MESSAGE_DROPPED: Option<ChannelTcpObserverMessageDroppedCallback> = None;

pub static mut NO_SOCKET_DROP: Option<ChannelTcpObserverCallback> = None;
pub static mut WRITE_DROP: Option<ChannelTcpObserverCallback> = None;
pub static mut DESTROY_OBSERVER: Option<VoidPointerCallback> = None;

pub struct FfiChannelTcpObserver {
    /// is a `shared_ptr<channel_tcp_observer> *`
    handle: *mut c_void,
}

impl FfiChannelTcpObserver {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

impl ChannelTcpObserver for FfiChannelTcpObserver {
    fn data_sent(&self, endpoint: &SocketAddr) {
        let dto = EndpointDto::from(endpoint);
        unsafe {
            DATA_SENT.expect("DATA_SENT missing")(self.handle, &dto);
        }
    }

    fn host_unreachable(&self) {
        unsafe {
            HOST_UNREACHABLE.expect("HOST_UNREACHABLE missing")(self.handle);
        }
    }

    fn message_sent(&self, message: &dyn Message) {
        unsafe {
            MESSAGE_SENT.expect("MESSAGE_SENT missing")(
                self.handle,
                MessageHandle::new(message.clone_box()),
            );
        }
    }

    fn message_dropped(&self, message: &dyn Message, buffer_size: usize) {
        unsafe {
            MESSAGE_DROPPED.expect("MESSAGE_DROPPED missing")(
                self.handle,
                MessageHandle::new(message.clone_box()),
                buffer_size,
            );
        }
    }

    fn no_socket_drop(&self) {
        unsafe {
            NO_SOCKET_DROP.expect("NO_SOCKET_DROP missing")(self.handle);
        }
    }

    fn write_drop(&self) {
        unsafe {
            WRITE_DROP.expect("WRITE_DROP missing")(self.handle);
        }
    }
}

impl Drop for FfiChannelTcpObserver {
    fn drop(&mut self) {
        unsafe {
            DESTROY_OBSERVER.expect("DESTROY_OBSERVER missing")(self.handle);
        }
    }
}

pub struct ChannelTcpObserverWeakPtr {
    /// `weak_ptr<channel_tcp_observer> *`
    handle: *mut c_void,
}

impl ChannelTcpObserverWeakPtr {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
}

impl IChannelTcpObserverWeakPtr for ChannelTcpObserverWeakPtr {
    fn lock(&self) -> Option<Arc<dyn ChannelTcpObserver>> {
        let shared_ptr_handle =
            unsafe { LOCK_OBSERVER.expect("LOCK_OBSERVER missing")(self.handle) };
        if shared_ptr_handle.is_null() {
            None
        } else {
            Some(Arc::new(FfiChannelTcpObserver::new(shared_ptr_handle)))
        }
    }
}

impl Drop for ChannelTcpObserverWeakPtr {
    fn drop(&mut self) {
        unsafe { DROP_WEAK_PTR.expect("DROP_WEAK_PTR missing")(self.handle) }
    }
}

impl Clone for ChannelTcpObserverWeakPtr {
    fn clone(&self) -> Self {
        Self {
            handle: unsafe { CLONE_WEAK_PTR.expect("CLONE_WEAK_PTR missing")(self.handle) },
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_data_sent(
    f: ChannelTcpObserverDataSentCallback,
) {
    DATA_SENT = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_host_unreachable(
    f: ChannelTcpObserverCallback,
) {
    HOST_UNREACHABLE = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_message_sent(
    f: ChannelTcpObserverMessageSentCallback,
) {
    MESSAGE_SENT = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_message_dropped(
    f: ChannelTcpObserverMessageDroppedCallback,
) {
    MESSAGE_DROPPED = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_no_socket_drop(
    f: ChannelTcpObserverCallback,
) {
    NO_SOCKET_DROP = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_write_drop(
    f: ChannelTcpObserverCallback,
) {
    WRITE_DROP = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_destroy(f: VoidPointerCallback) {
    DESTROY_OBSERVER = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_lock(
    f: ChannelTcpObserverLockWeakCallback,
) {
    LOCK_OBSERVER = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_drop_weak(f: VoidPointerCallback) {
    DROP_WEAK_PTR = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_clone_weak(
    f: ChannelTcpObserverWeakCloneCallback,
) {
    CLONE_WEAK_PTR = Some(f);
}
