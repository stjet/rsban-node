use std::{ffi::c_void, ops::Deref, sync::Arc, time::Duration};

use crate::{
    messages::MessageHandle,
    transport::{
        ChannelTcpSendBufferCallback, ChannelTcpSendCallback, ChannelTcpSendCallbackWrapper,
        EndpointDto, ReadCallbackWrapper, SendBufferCallbackWrapper, SocketDestroyContext,
        SocketHandle, SocketReadCallback,
    },
    StringDto, VoidPointerCallback,
};
use rsnano_node::{
    bootstrap::BootstrapClient,
    transport::{BufferDropPolicy, TrafficType},
};

use num_traits::FromPrimitive;

pub struct BootstrapClientHandle(pub Arc<BootstrapClient>);

impl Deref for BootstrapClientHandle {
    type Target = Arc<BootstrapClient>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_client_clone(
    handle: &BootstrapClientHandle,
) -> *mut BootstrapClientHandle {
    Box::into_raw(Box::new(BootstrapClientHandle(Arc::clone(&**handle))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_destroy(handle: *mut BootstrapClientHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_socket(
    handle: *mut BootstrapClientHandle,
) -> *mut SocketHandle {
    SocketHandle::new(Arc::clone((*handle).0.get_socket()))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_read(
    handle: *mut BootstrapClientHandle,
    size: usize,
    callback: SocketReadCallback,
    destroy_context: SocketDestroyContext,
    context: *mut c_void,
) {
    let cb_wrapper = ReadCallbackWrapper::new(callback, destroy_context, context);
    let cb = Box::new(move |ec, size| {
        cb_wrapper.execute(ec, size);
    });
    (*handle).0.read_async(size, cb)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_sample_block_rate(
    handle: *mut BootstrapClientHandle,
) -> f64 {
    (*handle).0.sample_block_rate()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_set_start_time(handle: *mut BootstrapClientHandle) {
    (*handle).0.set_start_time()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_elapsed_seconds(
    handle: *mut BootstrapClientHandle,
) -> f64 {
    (*handle).0.elapsed().as_secs_f64()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_receive_buffer_size(
    handle: *mut BootstrapClientHandle,
) -> usize {
    (*handle).0.receive_buffer_len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_receive_buffer(
    handle: *mut BootstrapClientHandle,
    buffer: *mut u8,
    len: usize,
) {
    let buffer = if buffer.is_null() {
        &mut []
    } else {
        std::slice::from_raw_parts_mut(buffer, len)
    };
    buffer.copy_from_slice(&(*handle).0.receive_buffer());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_send_buffer(
    handle: *mut BootstrapClientHandle,
    buffer: *const u8,
    len: usize,
    callback: ChannelTcpSendBufferCallback,
    delete_callback: VoidPointerCallback,
    callback_context: *mut c_void,
    policy: u8,
    traffic_type: u8,
) {
    let buffer = if buffer.is_null() {
        Arc::new(Vec::new())
    } else {
        Arc::new(std::slice::from_raw_parts(buffer, len).to_vec())
    };
    let callback_wrapper =
        SendBufferCallbackWrapper::new(callback, callback_context, delete_callback);
    let cb = Box::new(move |ec, size| {
        callback_wrapper.call(ec, size);
    });
    let policy = BufferDropPolicy::from_u8(policy).unwrap();
    let traffic_type = TrafficType::from_u8(traffic_type).unwrap();
    (*handle)
        .0
        .send_buffer(&buffer, Some(cb), policy, traffic_type);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_send(
    handle: *mut BootstrapClientHandle,
    msg: &MessageHandle,
    callback: ChannelTcpSendCallback,
    delete_callback: VoidPointerCallback,
    context: *mut c_void,
    policy: u8,
    traffic_type: u8,
) {
    let callback_wrapper = ChannelTcpSendCallbackWrapper::new(context, callback, delete_callback);
    let callback_box = Box::new(move |ec, size| {
        callback_wrapper.call(ec, size);
    });
    (*handle).0.send(
        &msg.message,
        Some(callback_box),
        BufferDropPolicy::from_u8(policy).unwrap(),
        TrafficType::from_u8(traffic_type).unwrap(),
    );
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_inc_block_count(
    handle: *mut BootstrapClientHandle,
) -> u64 {
    (*handle).0.inc_block_count()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_block_count(
    handle: *mut BootstrapClientHandle,
) -> u64 {
    (*handle).0.block_count()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_block_rate(
    handle: *mut BootstrapClientHandle,
) -> f64 {
    (*handle).0.block_rate()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_close_socket(handle: *mut BootstrapClientHandle) {
    (*handle).0.close_socket();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_remote_endpoint(
    handle: *mut BootstrapClientHandle,
    endpoint: *mut EndpointDto,
) {
    let ep = (*handle).0.remote_endpoint();
    *endpoint = EndpointDto::from(&ep);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_tcp_endpoint(
    handle: *mut BootstrapClientHandle,
    endpoint: *mut EndpointDto,
) {
    let ep = (*handle).0.tcp_endpoint();
    *endpoint = EndpointDto::from(&ep);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_channel_string(
    handle: *mut BootstrapClientHandle,
    result: *mut StringDto,
) {
    *result = StringDto::from((*handle).0.channel_string());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_set_timeout(
    handle: *mut BootstrapClientHandle,
    timeout_s: u64,
) {
    (*handle).0.set_timeout(Duration::from_secs(timeout_s));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_pending_stop(
    handle: *mut BootstrapClientHandle,
) -> bool {
    (*handle).0.pending_stop()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_hard_stop(
    handle: *mut BootstrapClientHandle,
) -> bool {
    (*handle).0.hard_stop()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_client_stop(
    handle: *mut BootstrapClientHandle,
    force: bool,
) {
    (*handle).0.stop(force);
}
