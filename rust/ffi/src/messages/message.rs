use crate::NetworkConstantsDto;
use rsnano_node::{
    config::NetworkConstants,
    messages::{MessageEnum, Payload, ProtocolInfo},
    transport::DeserializedMessage,
};

use std::ops::{Deref, DerefMut};

pub struct MessageHandle(pub DeserializedMessage);

impl MessageHandle {
    pub fn new(msg: MessageEnum) -> *mut Self {
        Box::into_raw(Box::new(Self(DeserializedMessage {
            message: msg.payload,
            protocol: msg.header.protocol,
        })))
    }
    pub fn new2(msg: DeserializedMessage) -> *mut Self {
        Box::into_raw(Box::new(Self(msg)))
    }
}

impl Deref for MessageHandle {
    type Target = DeserializedMessage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MessageHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_clone(handle: *mut MessageHandle) -> *mut MessageHandle {
    MessageHandle::new2((*handle).0.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_destroy(handle: *mut MessageHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_type(handle: *mut MessageHandle) -> u8 {
    (*handle).message.message_type() as u8
}

pub(crate) unsafe fn create_message_handle2(
    constants: *mut NetworkConstantsDto,
    f: impl FnOnce() -> Payload,
) -> *mut MessageHandle {
    let constants = NetworkConstants::try_from(&*constants).unwrap();
    let msg = DeserializedMessage::new(f(), constants.protocol_info());
    MessageHandle::new2(msg)
}

pub(crate) unsafe fn create_message_handle3(
    constants: *mut NetworkConstantsDto,
    f: impl FnOnce(ProtocolInfo) -> MessageEnum,
) -> *mut MessageHandle {
    let constants = NetworkConstants::try_from(&*constants).unwrap();
    MessageHandle::new(f(constants.protocol_info()))
}

pub(crate) fn message_handle_clone(handle: &MessageHandle) -> *mut MessageHandle {
    MessageHandle::new2(handle.deref().clone())
}
