use rsnano_core::{Account, BlockHash, PublicKey, Signature};

use crate::{
    copy_account_bytes, copy_hash_bytes, copy_signature_bytes, NetworkConstantsDto, StringDto,
};
use rsnano_node::messages::{
    NodeIdHandshake, NodeIdHandshakeQuery, NodeIdHandshakeResponse, V2Payload,
};

use super::{
    create_message_handle2, create_message_handle3, downcast_message, downcast_message_mut,
    message_handle_clone, MessageHandle, MessageHeaderHandle,
};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_create(
    constants: *mut NetworkConstantsDto,
    query: *const u8,
    resp_node_id: *const u8,
    resp_signature: *const u8,
    resp_salt: *const u8,
    resp_genesis: *const u8,
) -> *mut MessageHandle {
    let query = if !query.is_null() {
        let cookie = std::slice::from_raw_parts(query, 32).try_into().unwrap();
        Some(NodeIdHandshakeQuery { cookie })
    } else {
        None
    };

    let response = if !resp_node_id.is_null() && !resp_signature.is_null() {
        let node_id = Account::from_ptr(resp_node_id);
        let signature = Signature::from_ptr(resp_signature);
        let v2 = if resp_salt.is_null() {
            None
        } else {
            Some(V2Payload {
                salt: std::slice::from_raw_parts(resp_salt, 32)
                    .try_into()
                    .unwrap(),
                genesis: BlockHash::from_ptr(resp_genesis),
            })
        };
        Some(NodeIdHandshakeResponse {
            node_id,
            signature,
            v2,
        })
    } else {
        None
    };
    create_message_handle3(constants, move |protocol_info| {
        NodeIdHandshake::new(protocol_info, query, response)
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, NodeIdHandshake::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<NodeIdHandshake>(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_query(
    handle: *mut MessageHandle,
    result: *mut u8,
) -> bool {
    match &downcast_message::<NodeIdHandshake>(handle).query {
        Some(query) => {
            std::slice::from_raw_parts_mut(result, 32).copy_from_slice(&query.cookie);
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_response(
    handle: *mut MessageHandle,
    account: *mut u8,
    signature: *mut u8,
    is_v2: *mut bool,
    salt: *mut u8,
    genesis: *mut u8,
) -> bool {
    match &downcast_message::<NodeIdHandshake>(handle).response {
        Some(response) => {
            copy_account_bytes(response.node_id, account);
            copy_signature_bytes(&response.signature, signature);
            match &response.v2 {
                Some(v2) => {
                    let salt_slice = std::slice::from_raw_parts_mut(salt, 32);
                    salt_slice.copy_from_slice(&v2.salt);
                    copy_hash_bytes(v2.genesis, genesis);
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
pub unsafe extern "C" fn rsn_message_node_id_handshake_is_v2(handle: *mut MessageHandle) -> bool {
    downcast_message::<NodeIdHandshake>(handle).is_v2()
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
pub unsafe extern "C" fn rsn_message_node_id_handshake_size(
    header: *mut MessageHeaderHandle,
) -> usize {
    NodeIdHandshake::serialized_size(&*header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_to_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    (*result) = downcast_message_mut::<NodeIdHandshake>(handle)
        .to_string()
        .into();
}
