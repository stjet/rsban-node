use std::{ffi::c_void, ops::Deref, sync::Arc};

use rsnano_node::{
    config::NetworkConstants,
    messages::Message,
    transport::{MessageDeserializer, MessageDeserializerExt, SocketExtensions},
    utils::ErrorCode,
};

use crate::{
    core::BlockUniquerHandle, messages::MessageHandle, voting::VoteUniquerHandle, ErrorCodeDto,
    NetworkConstantsDto, VoidPointerCallback,
};

use super::{NetworkFilterHandle, SocketHandle};

pub type MessageReceivedCallback =
    unsafe extern "C" fn(*mut c_void, *const ErrorCodeDto, *mut MessageHandle);

pub struct MessageCallbackWrapper {
    callback: MessageReceivedCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
}

impl MessageCallbackWrapper {
    pub fn new(
        callback: MessageReceivedCallback,
        context: *mut c_void,
        delete_context: VoidPointerCallback,
    ) -> Self {
        Self {
            callback,
            context,
            delete_context,
        }
    }

    pub fn call(&self, ec: ErrorCode, msg: Option<Box<dyn Message>>) {
        let ec_dto = ErrorCodeDto::from(&ec);
        let message_handle = match msg {
            Some(m) => MessageHandle::new(m),
            None => std::ptr::null_mut(),
        };
        unsafe {
            (self.callback)(self.context, &ec_dto, message_handle);
            if !message_handle.is_null() {
                drop(Box::from_raw(message_handle));
            }
        }
    }
}

impl Drop for MessageCallbackWrapper {
    fn drop(&mut self) {
        unsafe {
            (self.delete_context)(self.context);
        }
    }
}

unsafe impl Send for MessageCallbackWrapper {}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_deserializer_read_socket(
    network_constants: *const NetworkConstantsDto,
    network_filter: *mut NetworkFilterHandle,
    block_uniquer: *mut BlockUniquerHandle,
    vote_uniquer: *mut VoteUniquerHandle,
    socket: *mut SocketHandle,
    message_callback: MessageReceivedCallback,
    message_callback_context: *mut c_void,
    delete_callback_context: VoidPointerCallback,
) {
    let network_constants = NetworkConstants::try_from(&*network_constants).unwrap();
    let network_filter = (*network_filter).deref().clone();
    let block_uniquer = (*block_uniquer).deref().clone();
    let vote_uniquer = (*vote_uniquer).deref().clone();
    let socket = (*socket).deref().clone();
    let read_op = Box::new(move |data, size, callback| {
        socket.read_impl(data, size, callback);
    });

    let callback = MessageCallbackWrapper::new(
        message_callback,
        message_callback_context,
        delete_callback_context,
    );

    let deserializer = Arc::new(MessageDeserializer::new(
        network_constants,
        network_filter,
        block_uniquer,
        vote_uniquer,
        read_op,
    ));

    deserializer.read(Box::new(move |ec, msg| {
        callback.call(ec, msg);
    }))
}
