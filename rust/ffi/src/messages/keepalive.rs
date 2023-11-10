use super::{create_message_handle2, message_handle_clone, MessageHandle};
use crate::{transport::EndpointDto, NetworkConstantsDto, StringDto};
use rsnano_node::messages::{Keepalive, Message};
use std::net::SocketAddrV6;

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle2(constants, || Message::Keepalive(Default::default()))
}

#[no_mangle]
pub extern "C" fn rsn_message_keepalive_clone(handle: &MessageHandle) -> *mut MessageHandle {
    message_handle_clone(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_peers(
    handle: &MessageHandle,
    result: *mut EndpointDto,
) {
    let dtos = std::slice::from_raw_parts_mut(result, 8);
    let Message::Keepalive(payload) = &handle.message else {
        panic!("not a keepalive payload")
    };
    let peers: Vec<_> = payload.peers.iter().map(EndpointDto::from).collect();
    dtos.clone_from_slice(&peers);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_set_peers(
    handle: &mut MessageHandle,
    result: *const EndpointDto,
) {
    let dtos = std::slice::from_raw_parts(result, 8);
    let peers: [SocketAddrV6; 8] = dtos
        .iter()
        .map(SocketAddrV6::from)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    handle.message = Message::Keepalive(Keepalive { peers });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_size() -> usize {
    Keepalive::serialized_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_to_string(
    handle: &MessageHandle,
    result: *mut StringDto,
) {
    let s = handle.message.to_string();
    *result = s.into()
}
