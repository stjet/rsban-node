use super::{message_handle_clone, MessageHandle};
use crate::StringDto;
use rsnano_core::{BlockHash, PublicKey, Signature};
use rsnano_messages::{Message, NodeIdHandshake, NodeIdHandshakeResponse, V2Payload};

#[no_mangle]
pub extern "C" fn rsn_message_node_id_handshake_clone(
    handle: &MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone(handle)
}

fn get_payload(handle: &MessageHandle) -> &NodeIdHandshake {
    let Message::NodeIdHandshake(payload) = &handle.message else {
        panic!("not a node_id_handshake")
    };
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_query(
    handle: &MessageHandle,
    result: *mut u8,
) -> bool {
    match &get_payload(handle).query {
        Some(query) => {
            std::slice::from_raw_parts_mut(result, 32).copy_from_slice(&query.cookie);
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_response(
    handle: &MessageHandle,
    account: *mut u8,
    signature: *mut u8,
    is_v2: *mut bool,
    salt: *mut u8,
    genesis: *mut u8,
) -> bool {
    match &get_payload(handle).response {
        Some(response) => {
            response.node_id.copy_bytes(account);
            response.signature.copy_bytes(signature);
            match &response.v2 {
                Some(v2) => {
                    let salt_slice = std::slice::from_raw_parts_mut(salt, 32);
                    salt_slice.copy_from_slice(&v2.salt);
                    v2.genesis.copy_bytes(genesis);
                    *is_v2 = true;
                }
                None => {
                    *is_v2 = false;
                }
            }
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_is_v2(handle: &MessageHandle) -> bool {
    get_payload(handle).is_v2
}

#[repr(C)]
pub struct HandshakeResponseDto {
    pub node_id: [u8; 32],
    pub signature: [u8; 64],
    pub v2: bool,
    pub salt: [u8; 32],
    pub genesis: [u8; 32],
}

impl From<&HandshakeResponseDto> for NodeIdHandshakeResponse {
    fn from(value: &HandshakeResponseDto) -> Self {
        NodeIdHandshakeResponse {
            node_id: PublicKey::from_bytes(value.node_id),
            signature: Signature::from_bytes(value.signature),
            v2: if value.v2 {
                Some(V2Payload {
                    genesis: BlockHash::from_bytes(value.genesis),
                    salt: value.salt,
                })
            } else {
                None
            },
        }
    }
}

impl From<NodeIdHandshakeResponse> for HandshakeResponseDto {
    fn from(value: NodeIdHandshakeResponse) -> Self {
        Self {
            node_id: *value.node_id.as_bytes(),
            signature: *value.signature.as_bytes(),
            v2: value.v2.is_some(),
            salt: if let Some(v2) = &value.v2 {
                v2.salt
            } else {
                [0; 32]
            },
            genesis: if let Some(v2) = &value.v2 {
                *v2.genesis.as_bytes()
            } else {
                [0; 32]
            },
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_to_string(
    handle: &MessageHandle,
    result: *mut StringDto,
) {
    (*result) = handle.message.to_string().into();
}
