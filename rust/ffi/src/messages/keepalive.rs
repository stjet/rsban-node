use std::net::SocketAddr;

use rsnano_node::messages::Keepalive;

use super::{
    create_message_handle, create_message_handle2, downcast_message, downcast_message_mut,
    message_handle_clone, MessageHandle, MessageHeaderHandle,
};
use crate::{transport::EndpointDto, NetworkConstantsDto, StringDto};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_create(
    constants: *mut NetworkConstantsDto,
    version_using: i16,
) -> *mut MessageHandle {
    create_message_handle(constants, |consts| {
        if version_using < 0 {
            Keepalive::new(consts)
        } else {
            Keepalive::with_version_using(consts, version_using as u8)
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, Keepalive::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<Keepalive>(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_peers(
    handle: *mut MessageHandle,
    result: *mut EndpointDto,
) {
    let dtos = std::slice::from_raw_parts_mut(result, 8);
    let peers: Vec<_> = downcast_message::<Keepalive>(handle)
        .peers()
        .iter()
        .map(EndpointDto::from)
        .collect();
    dtos.clone_from_slice(&peers);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_set_peers(
    handle: *mut MessageHandle,
    result: *const EndpointDto,
) {
    let dtos = std::slice::from_raw_parts(result, 8);
    let peers: [SocketAddr; 8] = dtos
        .iter()
        .map(SocketAddr::from)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    downcast_message_mut::<Keepalive>(handle).set_peers(&peers);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_size() -> usize {
    Keepalive::serialized_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_to_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    let s = downcast_message_mut::<Keepalive>(handle).to_string();
    *result = s.into()
}
