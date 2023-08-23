use std::ffi::c_void;

use rsnano_core::{Account, BlockHash, KeyPair, PublicKey, Signature};

use crate::{
    copy_account_bytes, copy_hash_bytes, copy_signature_bytes, utils::FfiStream,
    NetworkConstantsDto, StringDto,
};
use rsnano_node::messages::{
    Message, NodeIdHandshake, NodeIdHandshakeQuery, NodeIdHandshakeResponse, V2Payload,
};

use super::{
    create_message_handle, create_message_handle2, downcast_message, downcast_message_mut,
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
    create_message_handle(constants, move |consts| {
        NodeIdHandshake::new(consts, query, response)
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
pub unsafe extern "C" fn rsn_message_node_id_handshake_deserialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    downcast_message_mut::<NodeIdHandshake>(handle)
        .deserialize(&mut stream)
        .is_ok()
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
pub unsafe extern "C" fn rsn_message_node_id_handshake_response_create(
    cookie: *const u8,
    priv_key: *const u8,
    genesis: *const u8,
    result: *mut HandshakeResponseDto,
) {
    let cookie = std::slice::from_raw_parts(cookie, 32).try_into().unwrap();
    let key = KeyPair::from_priv_key_bytes(std::slice::from_raw_parts(priv_key, 32)).unwrap();
    let response = if genesis.is_null() {
        NodeIdHandshakeResponse::new_v1(cookie, &key)
    } else {
        let genesis = BlockHash::from_ptr(genesis);
        NodeIdHandshakeResponse::new_v2(cookie, &key, genesis)
    };
    *result = response.into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_response_validate(
    cookie: *const u8,
    node_id: *const u8,
    signature: *const u8,
    salt: *const u8,
    genesis: *const u8,
) -> bool {
    let node_id = Account::from_ptr(node_id);
    let signature = Signature::from_ptr(signature);
    let cookie = std::slice::from_raw_parts(cookie, 32).try_into().unwrap();

    let v2 = if !salt.is_null() {
        Some(V2Payload {
            salt: std::slice::from_raw_parts(salt, 32).try_into().unwrap(),
            genesis: BlockHash::from_ptr(genesis),
        })
    } else {
        None
    };

    let response = NodeIdHandshakeResponse {
        node_id,
        signature,
        v2,
    };
    response.validate(&cookie).is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_size(
    header: *mut MessageHeaderHandle,
) -> usize {
    NodeIdHandshake::serialized_size(&*header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_serialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    downcast_message::<NodeIdHandshake>(handle)
        .serialize(&mut stream)
        .is_ok()
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
