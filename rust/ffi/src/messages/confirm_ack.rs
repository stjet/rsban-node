use std::{ffi::c_void, ops::Deref, sync::Arc};

use crate::{
    utils::FfiStream,
    voting::{VoteHandle, VoteUniquerHandle},
    NetworkConstantsDto, StringDto,
};
use rsnano_node::{
    messages::{ConfirmAckPayload, MessageEnum, Payload},
    voting::Vote,
};

use super::{
    create_message_handle2, create_message_handle3, downcast_message, message_handle_clone,
    MessageHandle, MessageHeaderHandle,
};

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
pub unsafe extern "C" fn rsn_message_confirm_ack_create2(
    header: *mut MessageHeaderHandle,
    stream: *mut c_void,
    uniquer: *mut VoteUniquerHandle,
    is_error: *mut bool,
) -> *mut MessageHandle {
    create_message_handle2(header, |hdr| {
        let mut stream = FfiStream::new(stream);
        let uniquer = if uniquer.is_null() {
            None
        } else {
            Some((*uniquer).deref().as_ref())
        };

        match MessageEnum::deserialize(&mut stream, hdr, 0, None, uniquer) {
            Ok(i) => i,
            Err(_) => {
                *is_error = true;
                //workaround to prevent nullptr:
                MessageEnum::new_confirm_ack(&Default::default(), Arc::new(Vote::null()))
            }
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<MessageEnum>(handle)
}

unsafe fn get_payload(handle: *mut MessageHandle) -> &'static ConfirmAckPayload {
    let msg = downcast_message::<MessageEnum>(handle);
    let Payload::ConfirmAck(payload) = &msg.payload else {panic!("not a confirm_ack message")};
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_vote(
    handle: *mut MessageHandle,
) -> *mut VoteHandle {
    let vote = get_payload(handle).vote.clone();
    Box::into_raw(Box::new(VoteHandle::new(vote)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_size(count: usize) -> usize {
    ConfirmAckPayload::serialized_size(count as u8)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_to_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    (*result) = downcast_message::<MessageEnum>(handle).to_string().into();
}
