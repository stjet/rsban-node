use super::{FfiPropertyTreeWriter, StringDto, StringHandle};
use crate::{from_topic, to_topic, Message, MessageBuilder};
use num::FromPrimitive;
use std::{
    ffi::{c_void, CStr, CString},
    os::raw::c_char,
    time::Duration,
};

#[repr(C)]
pub struct MessageDto {
    pub topic: u8,
    pub contents: *mut c_void,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_set_common_fields(message: *mut MessageDto) {
    let dto = &mut (*message);
    let mut message = Message {
        topic: FromPrimitive::from_u8(dto.topic).unwrap(),
        contents: Box::new(FfiPropertyTreeWriter::new_borrowed(dto.contents)),
    };
    MessageBuilder::set_common_fields(&mut message).unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_from_topic(topic: u8, result: *mut StringDto) {
    let topic_string = Box::new(StringHandle(
        CString::new(from_topic(FromPrimitive::from_u8(topic).unwrap())).unwrap(),
    ));
    (*result).value = topic_string.0.as_ptr();
    (*result).handle = Box::into_raw(topic_string);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_to_topic(topic: *const c_char) -> u8 {
    to_topic(CStr::from_ptr(topic).to_string_lossy()) as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_bootstrap_started(
    id: *const c_char,
    mode: *const c_char,
    result: *mut MessageDto,
) {
    let message = MessageBuilder::bootstrap_started(
        &CStr::from_ptr(id).to_string_lossy(),
        &CStr::from_ptr(mode).to_string_lossy(),
    )
    .unwrap();

    set_message_dto(result, message);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_builder_bootstrap_exited(
    id: *const c_char,
    mode: *const c_char,
    duration_s: u64,
    total_blocks: u64,
    result: *mut MessageDto,
) {
    let message = MessageBuilder::bootstrap_exited(
        &CStr::from_ptr(id).to_string_lossy(),
        &CStr::from_ptr(mode).to_string_lossy(),
        Duration::from_secs(duration_s),
        total_blocks,
    )
    .unwrap();

    set_message_dto(result, message);
}

unsafe fn set_message_dto(result: *mut MessageDto, message: Message) {
    (*result).topic = message.topic as u8;
    (*result).contents = message
        .contents
        .as_any()
        .downcast_ref::<FfiPropertyTreeWriter>()
        .unwrap()
        .handle;
    // Forget the message, so that the property_tree handle won't get deleted.
    // The caller of this function is responsable for calling delete on the handle.
    std::mem::forget(message);
}
