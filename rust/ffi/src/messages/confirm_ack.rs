use super::{create_message_handle3, message_handle_clone, MessageHandle};
use crate::{voting::VoteHandle, NetworkConstantsDto, StringDto};
use rsnano_node::messages::{ConfirmAckPayload, MessageEnum, Payload};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_create(
    constants: *mut NetworkConstantsDto,
    vote: *mut VoteHandle,
) -> *mut MessageHandle {
    create_message_handle3(constants, |consts| {
        let vote = (*vote).clone();
        MessageEnum::new_confirm_ack(consts, vote)
    })
}

#[no_mangle]
pub extern "C" fn rsn_message_confirm_ack_clone(handle: &MessageHandle) -> *mut MessageHandle {
    message_handle_clone(handle)
}

unsafe fn get_payload(handle: &MessageHandle) -> &ConfirmAckPayload {
    let Payload::ConfirmAck(payload) = &handle.message else {panic!("not a confirm_ack message")};
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_vote(handle: &MessageHandle) -> *mut VoteHandle {
    let vote = get_payload(handle).vote.clone();
    Box::into_raw(Box::new(VoteHandle::new(vote)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_size(count: usize) -> usize {
    ConfirmAckPayload::serialized_size(count as u8)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_to_string(
    handle: &MessageHandle,
    result: *mut StringDto,
) {
    (*result) = handle.message.to_string().into();
}
