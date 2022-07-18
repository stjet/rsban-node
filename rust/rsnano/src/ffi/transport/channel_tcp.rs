use num::FromPrimitive;

use super::{
    channel::{as_tcp_channel, ChannelHandle, ChannelType},
    socket::SocketHandle,
    EndpointDto,
};
use crate::{
    ffi::{
        io_context::FfiIoContext, messages::MessageHandle, BandwidthLimiterHandle, DestroyCallback,
        ErrorCodeDto,
    },
    messages::Message,
    transport::{BufferDropPolicy, ChannelTcp, ChannelTcpObserver, TcpChannelData},
    utils::ErrorCode,
};
use std::{
    ffi::c_void,
    net::SocketAddr,
    ops::Deref,
    sync::{Arc, MutexGuard},
};

#[no_mangle]
/// observer is `weak_ptr<channel_tcp_observer> *`
/// io_ctx is `boost::asio::io_context *`
pub unsafe extern "C" fn rsn_channel_tcp_create(
    now: u64,
    socket: *mut SocketHandle,
    observer: *mut c_void,
    limiter: *const BandwidthLimiterHandle,
    io_ctx: *mut c_void,
) -> *mut ChannelHandle {
    let observer = ChannelTcpObserverWeakPtr::new(observer);
    let limiter = Arc::clone(&*limiter);
    let io_ctx = Arc::new(FfiIoContext::new(io_ctx));
    Box::into_raw(Box::new(ChannelHandle::new(Arc::new(ChannelType::Tcp(
        ChannelTcp::new((*socket).deref(), now, observer, limiter, io_ctx),
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

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_endpoint(
    handle: *mut ChannelHandle,
    endpoint: *mut EndpointDto,
) {
    (*endpoint) = EndpointDto::from(as_tcp_channel(handle).endpoint())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_set_endpoint(handle: *mut ChannelHandle) {
    as_tcp_channel(handle).set_endpoint();
}

pub type ChannelTcpSendBufferCallback =
    unsafe extern "C" fn(*mut c_void, *const ErrorCodeDto, usize);

pub struct SendBufferCallbackWrapper {
    callback: ChannelTcpSendBufferCallback,
    /// `std::function<error_code const&, size_t>*`
    context: *mut c_void,
    delete_callback: DestroyCallback,
}

impl SendBufferCallbackWrapper {
    pub fn new(
        callback: ChannelTcpSendBufferCallback,
        context: *mut c_void,
        delete_callback: DestroyCallback,
    ) -> Self {
        Self {
            callback,
            context,
            delete_callback,
        }
    }

    pub fn call(&self, ec: ErrorCode, size: usize) {
        let ec_dto = ErrorCodeDto::from(&ec);
        unsafe {
            (self.callback)(self.context, &ec_dto, size);
        }
    }
}

impl Drop for SendBufferCallbackWrapper {
    fn drop(&mut self) {
        unsafe {
            (self.delete_callback)(self.context);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_send_buffer(
    handle: *mut ChannelHandle,
    buffer: *const u8,
    buffer_len: usize,
    callback: ChannelTcpSendBufferCallback,
    delete_callback: DestroyCallback,
    callback_context: *mut c_void,
    policy: u8,
) {
    let buffer = Arc::new(std::slice::from_raw_parts(buffer, buffer_len).to_vec());
    let callback_wrapper =
        SendBufferCallbackWrapper::new(callback, callback_context, delete_callback);
    let cb = Box::new(move |ec, size| {
        callback_wrapper.call(ec, size);
    });
    let policy = BufferDropPolicy::from_u8(policy).unwrap();
    as_tcp_channel(handle).send_buffer(&buffer, Some(cb), policy);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_socket(handle: *mut ChannelHandle) -> *mut SocketHandle {
    let tcp = as_tcp_channel(handle);
    match tcp.socket() {
        Some(s) => SocketHandle::new(s),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_network_version(handle: *mut ChannelHandle) -> u8 {
    let tcp = as_tcp_channel(handle);
    tcp.network_version()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_network_set_version(
    handle: *mut ChannelHandle,
    version: u8,
) {
    let tcp = as_tcp_channel(handle);
    tcp.set_network_version(version)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_eq(a: *mut ChannelHandle, b: *mut ChannelHandle) -> bool {
    as_tcp_channel(a).eq(as_tcp_channel(b))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_max(handle: *mut ChannelHandle) -> bool {
    as_tcp_channel(handle).max()
}

pub type ChannelTcpSendCallback = unsafe extern "C" fn(*mut c_void, *const ErrorCodeDto, usize);

struct ChannelTcpSendCallbackWrapper {
    context: *mut c_void,
    callback: ChannelTcpSendCallback,
    delete: DestroyCallback,
}

impl ChannelTcpSendCallbackWrapper {
    fn new(
        context: *mut c_void,
        callback: ChannelTcpSendCallback,
        delete: DestroyCallback,
    ) -> Self {
        Self {
            context,
            callback,
            delete,
        }
    }

    fn call(&self, ec: ErrorCode, size: usize) {
        let ec_dto = ErrorCodeDto::from(&ec);
        unsafe {
            (self.callback)(self.context, &ec_dto, size);
        }
    }
}

impl Drop for ChannelTcpSendCallbackWrapper {
    fn drop(&mut self) {
        unsafe {
            (self.delete)(self.context);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_channel_tcp_send(
    handle: *mut ChannelHandle,
    msg: *mut MessageHandle,
    callback: ChannelTcpSendCallback,
    delete_callback: DestroyCallback,
    context: *mut c_void,
    policy: u8,
) {
    let callback_wrapper = ChannelTcpSendCallbackWrapper::new(context, callback, delete_callback);
    let callback_box = Box::new(move |ec, size| {
        callback_wrapper.call(ec, size);
    });
    as_tcp_channel(handle).send(
        (*msg).as_ref(),
        Some(callback_box),
        BufferDropPolicy::from_u8(policy).unwrap(),
    );
}

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

pub type ChannelTcpObserverDataSentCallback = unsafe extern "C" fn(*mut c_void, *const EndpointDto);
static mut DATA_SENT: Option<ChannelTcpObserverDataSentCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_data_sent(
    f: ChannelTcpObserverDataSentCallback,
) {
    DATA_SENT = Some(f);
}

pub type ChannelTcpObserverCallback = unsafe extern "C" fn(*mut c_void);
static mut HOST_UNREACHABLE: Option<ChannelTcpObserverCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_host_unreachable(
    f: ChannelTcpObserverCallback,
) {
    HOST_UNREACHABLE = Some(f);
}

pub type ChannelTcpObserverMessageSentCallback =
    unsafe extern "C" fn(*mut c_void, message: *mut MessageHandle);
static mut MESSAGE_SENT: Option<ChannelTcpObserverMessageSentCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_message_sent(
    f: ChannelTcpObserverMessageSentCallback,
) {
    MESSAGE_SENT = Some(f);
}

pub type ChannelTcpObserverMessageDroppedCallback =
    unsafe extern "C" fn(*mut c_void, message: *mut MessageHandle, usize);
static mut MESSAGE_DROPPED: Option<ChannelTcpObserverMessageDroppedCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_message_dropped(
    f: ChannelTcpObserverMessageDroppedCallback,
) {
    MESSAGE_DROPPED = Some(f);
}

static mut NO_SOCKET_DROP: Option<ChannelTcpObserverCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_no_socket_drop(
    f: ChannelTcpObserverCallback,
) {
    NO_SOCKET_DROP = Some(f);
}

static mut WRITE_DROP: Option<ChannelTcpObserverCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_write_drop(
    f: ChannelTcpObserverCallback,
) {
    WRITE_DROP = Some(f);
}

static mut DESTROY_OBSERVER: Option<DestroyCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_destroy(f: DestroyCallback) {
    DESTROY_OBSERVER = Some(f);
}

pub struct ChannelTcpObserverWeakPtr {
    /// `weak_ptr<channel_tcp_observer> *`
    handle: *mut c_void,
}

impl ChannelTcpObserverWeakPtr {
    pub fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }
    pub fn lock(&self) -> Option<Arc<dyn ChannelTcpObserver>> {
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

/// input is a `weak_ptr<channel_tcp_observer> *`
/// output is a `shared_ptr<channel_tcp_observer> *` or `nullptr`
pub type ChannelTcpObserverLockWeakCallback = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
static mut LOCK_OBSERVER: Option<ChannelTcpObserverLockWeakCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_lock(
    f: ChannelTcpObserverLockWeakCallback,
) {
    LOCK_OBSERVER = Some(f);
}

static mut DROP_WEAK_PTR: Option<DestroyCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_drop_weak(f: DestroyCallback) {
    DROP_WEAK_PTR = Some(f);
}

/// clones a `weak_ptr<channel_tcp_observer> *`
pub type ChannelTcpObserverWeakCloneCallback = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
static mut CLONE_WEAK_PTR: Option<ChannelTcpObserverWeakCloneCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_channel_tcp_observer_clone_weak(
    f: ChannelTcpObserverWeakCloneCallback,
) {
    CLONE_WEAK_PTR = Some(f);
}
