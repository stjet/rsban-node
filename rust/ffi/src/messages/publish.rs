use rsnano_node::messages::{MessageEnum, Payload, PublishPayload};
use std::{ops::Deref, sync::Arc};

use super::{
    create_message_handle2, create_message_handle3, message_handle_clone, MessageHandle,
    MessageHeaderHandle,
};
use crate::{core::BlockHandle, NetworkConstantsDto, StringDto};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_create(
    constants: *mut NetworkConstantsDto,
    block: &BlockHandle,
) -> *mut MessageHandle {
    create_message_handle3(constants, |protocol_info| {
        let block = Arc::clone((*block).deref());
        MessageEnum::new_publish(protocol_info, block)
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_create2(
    header: *mut MessageHeaderHandle,
    digest: *const u8,
) -> *mut MessageHandle {
    let digest = u128::from_be_bytes(std::slice::from_raw_parts(digest, 16).try_into().unwrap());
    create_message_handle2(header, |header| MessageEnum {
        header,
        payload: Payload::Publish(PublishPayload {
            block: None,
            digest,
        }),
    })
}

#[no_mangle]
pub extern "C" fn rsn_message_publish_clone(handle: &MessageHandle) -> *mut MessageHandle {
    message_handle_clone(handle)
}

fn get_publish_payload(handle: &MessageHandle) -> &PublishPayload {
    let Payload::Publish(payload) = &handle.payload else {panic!("not a payload message")};
    payload
}

fn get_publish_payload_mut(handle: &mut MessageHandle) -> &mut PublishPayload {
    let Payload::Publish(payload) = &mut handle.payload else {panic!("not a payload message")};
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_block(handle: &MessageHandle) -> *mut BlockHandle {
    match &get_publish_payload(handle).block {
        Some(b) => Box::into_raw(Box::new(BlockHandle(b.clone()))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_digest(handle: &MessageHandle, result: *mut u8) {
    let result_slice = std::slice::from_raw_parts_mut(result, 16);
    let digest = get_publish_payload(handle).digest;
    result_slice.copy_from_slice(&digest.to_be_bytes());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_set_digest(
    handle: &mut MessageHandle,
    digest: *const u8,
) {
    let bytes = std::slice::from_raw_parts(digest, 16);
    let digest = u128::from_be_bytes(bytes.try_into().unwrap());
    get_publish_payload_mut(handle).digest = digest;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_to_string(
    handle: &mut MessageHandle,
    result: *mut StringDto,
) {
    (*result) = handle.to_string().into();
}
