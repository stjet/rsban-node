use super::{FfiPropertyTreeWriter, StringDto, StringHandle};
use crate::websocket::{from_topic, Message, MessageBuilder};
use num::FromPrimitive;
use std::ffi::{c_void, CString};

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
