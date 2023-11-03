use super::MessageHeaderHandle;
use crate::NetworkConstantsDto;
use rsnano_node::{
    config::NetworkConstants,
    messages::{Message, MessageEnum, MessageHeader, ProtocolInfo},
};

use std::ops::{Deref, DerefMut};

pub struct MessageHandle(pub Box<MessageEnum>);

impl MessageHandle {
    pub fn new(msg: Box<MessageEnum>) -> *mut Self {
        Box::into_raw(Box::new(Self(msg)))
    }
}

impl Deref for MessageHandle {
    type Target = Box<MessageEnum>;

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
pub unsafe extern "C" fn rsn_message_header(
    handle: *mut MessageHandle,
) -> *mut MessageHeaderHandle {
    Box::into_raw(Box::new(MessageHeaderHandle::new(
        (*handle).0.header().clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_set_header(
    handle: *mut MessageHandle,
    header: *mut MessageHeaderHandle,
) {
    (*handle).0.set_header((*header).deref())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_clone(handle: *mut MessageHandle) -> *mut MessageHandle {
    MessageHandle::new((*handle).0.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_destroy(handle: *mut MessageHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_type(handle: *mut MessageHandle) -> u8 {
    (*handle).message_type() as u8
}

pub(crate) unsafe fn create_message_handle2(
    header: *mut MessageHeaderHandle,
    f: impl FnOnce(MessageHeader) -> MessageEnum,
) -> *mut MessageHandle {
    let msg = f((*header).deref().clone());
    MessageHandle::new(Box::new(msg))
}

pub(crate) unsafe fn create_message_handle3(
    constants: *mut NetworkConstantsDto,
    f: impl FnOnce(&ProtocolInfo) -> MessageEnum,
) -> *mut MessageHandle {
    let constants = NetworkConstants::try_from(&*constants).unwrap();
    MessageHandle::new(Box::new(f(&constants.protocol_info())))
}

pub(crate) fn message_handle_clone(handle: &MessageHandle) -> *mut MessageHandle {
    MessageHandle::new(handle.deref().clone())
}
