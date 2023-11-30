use std::{ops::Deref, sync::Arc};

use super::{create_message_handle2, message_handle_clone, MessageHandle};
use crate::{consensus::VoteHandle, NetworkConstantsDto, StringDto};
use rsnano_messages::{ConfirmAck, Message};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_create(
    constants: *mut NetworkConstantsDto,
    vote: &VoteHandle,
) -> *mut MessageHandle {
    create_message_handle2(constants, || {
        let vote = vote.0.deref().clone();
        Message::ConfirmAck(ConfirmAck { vote })
    })
}

#[no_mangle]
pub extern "C" fn rsn_message_confirm_ack_clone(handle: &MessageHandle) -> *mut MessageHandle {
    message_handle_clone(handle)
}

unsafe fn get_payload(handle: &MessageHandle) -> &ConfirmAck {
    let Message::ConfirmAck(payload) = &handle.message else {
        panic!("not a confirm_ack message")
    };
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_vote(handle: &MessageHandle) -> *mut VoteHandle {
    let vote = get_payload(handle).vote.clone();
    Box::into_raw(Box::new(VoteHandle::new(Arc::new(vote))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_to_string(
    handle: &MessageHandle,
    result: *mut StringDto,
) {
    (*result) = handle.message.to_string().into();
}
