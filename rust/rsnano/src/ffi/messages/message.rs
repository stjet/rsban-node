use super::MessageHeaderHandle;
use crate::{
    ffi::{transport::EndpointDto, NetworkConstantsDto},
    messages::{
        BulkPull, BulkPullAccount, BulkPush, ConfirmAck, ConfirmReq, FrontierReq, Keepalive,
        Message, MessageHeader, NodeIdHandshake, Publish, TelemetryAck, TelemetryReq,
    },
    NetworkConstants,
};
use std::{net::SocketAddr, ops::Deref};

pub struct MessageHandle(Box<dyn Message>);

#[no_mangle]
pub unsafe extern "C" fn rsn_message_header(
    handle: *mut MessageHandle,
) -> *mut MessageHeaderHandle {
    Box::into_raw(Box::new(MessageHeaderHandle::new(
        (*handle).0.header().clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_set_header(
    handle: *mut MessageHandle,
    header: *mut MessageHeaderHandle,
) {
    (*handle).0.set_header((*header).deref())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_destroy(handle: *mut MessageHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_create(
    constants: *mut NetworkConstantsDto,
    version_using: i16,
) -> *mut MessageHandle {
    create_message_handle(constants, |consts| {
        if version_using < 0 {
            Keepalive::new(&consts)
        } else {
            Keepalive::with_version_using(&consts, version_using as u8)
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
        .map(|x| EndpointDto::from(x))
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
        .map(|x| SocketAddr::from(x))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    downcast_message_mut::<Keepalive>(handle).set_peers(&peers);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, Publish::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, Publish::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<Publish>(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, ConfirmReq::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, ConfirmReq::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<ConfirmReq>(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, ConfirmAck::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, ConfirmAck::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<ConfirmAck>(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, FrontierReq::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_frontier_req_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, FrontierReq::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, BulkPull::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, BulkPull::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_account_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, BulkPullAccount::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_account_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, BulkPullAccount::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_push_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, BulkPush::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_push_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, BulkPush::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, TelemetryReq::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, TelemetryReq::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_req_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<TelemetryReq>(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_ack_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, TelemetryAck::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_ack_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, TelemetryAck::with_header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_telemetry_ack_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<TelemetryAck>(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_node_id_handshake_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, NodeIdHandshake::new)
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

unsafe fn create_message_handle<T: 'static + Message>(
    constants: *mut NetworkConstantsDto,
    f: impl FnOnce(&NetworkConstants) -> T,
) -> *mut MessageHandle {
    let constants = NetworkConstants::try_from(&*constants).unwrap();
    Box::into_raw(Box::new(MessageHandle(Box::new(f(&constants)))))
}

unsafe fn create_message_handle2<T: 'static + Message>(
    header: *mut MessageHeaderHandle,
    f: impl FnOnce(&MessageHeader) -> T,
) -> *mut MessageHandle {
    let msg = f((*header).deref());
    Box::into_raw(Box::new(MessageHandle(Box::new(msg))))
}

unsafe fn message_handle_clone<T: 'static + Message + Clone>(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    let msg = downcast_message::<T>(handle);
    Box::into_raw(Box::new(MessageHandle(Box::new(msg.clone()))))
}

unsafe fn downcast_message<T: 'static + Message>(handle: *mut MessageHandle) -> &'static T {
    (*handle).0.as_any().downcast_ref::<T>().unwrap()
}

unsafe fn downcast_message_mut<T: 'static + Message>(handle: *mut MessageHandle) -> &'static mut T {
    (*handle).0.as_any_mut().downcast_mut::<T>().unwrap()
}
