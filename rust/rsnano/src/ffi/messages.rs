use num::FromPrimitive;

use crate::messages::MessageType;

use super::StringDto;

#[no_mangle]
pub unsafe extern "C" fn rsn_message_type_to_string(msg_type: u8, result: *mut StringDto) {
    (*result) = match MessageType::from_u8(msg_type) {
        Some(msg_type) => msg_type.as_str().into(),
        None => "n/a".into(),
    }
}
