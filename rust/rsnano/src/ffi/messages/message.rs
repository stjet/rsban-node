use super::MessageHeaderHandle;
use crate::{
    ffi::{
        transport::EndpointDto, voting::VoteHandle, BlockHandle, BlockUniquerHandle, FfiStream,
        NetworkConstantsDto, StringDto,
    },
    messages::{
        BulkPull, BulkPullAccount, BulkPush, ConfirmAck, ConfirmReq, FrontierReq, Keepalive,
        Message, MessageHeader, NodeIdHandshake, Publish, TelemetryAck, TelemetryReq,
    },
    BlockHash, BlockType, NetworkConstants, Root,
};
use num_traits::FromPrimitive;
use std::{ffi::c_void, net::SocketAddr, ops::Deref, sync::Arc};

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
pub unsafe extern "C" fn rsn_message_keepalive_serialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    downcast_message::<Keepalive>(handle)
        .serialize(&mut stream)
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_deserialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    downcast_message_mut::<Keepalive>(handle)
        .deserialize(&mut stream)
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_keepalive_size() -> usize {
    Keepalive::size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_create(
    constants: *mut NetworkConstantsDto,
    block: *mut BlockHandle,
) -> *mut MessageHandle {
    create_message_handle(constants, |consts| {
        let block = (*block).block.clone();
        Publish::new(consts, block)
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_create2(
    header: *mut MessageHeaderHandle,
    digest: *const u8,
) -> *mut MessageHandle {
    let digest = u128::from_be_bytes(std::slice::from_raw_parts(digest, 16).try_into().unwrap());
    create_message_handle2(header, |consts| Publish::with_header(consts, digest))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<Publish>(handle)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_serialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    downcast_message::<Publish>(handle)
        .serialize(&mut stream)
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_deserialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
    uniquer: *mut BlockUniquerHandle,
) -> bool {
    let mut stream = FfiStream::new(stream);
    let uniquer = if uniquer.is_null() {
        None
    } else {
        Some((*uniquer).deref())
    };
    downcast_message_mut::<Publish>(handle)
        .deserialize(&mut stream, uniquer)
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_block(handle: *mut MessageHandle) -> *mut BlockHandle {
    match &downcast_message::<Publish>(handle).block {
        Some(b) => Box::into_raw(Box::new(BlockHandle::new(b.clone()))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_digest(handle: *mut MessageHandle, result: *mut u8) {
    let result_slice = std::slice::from_raw_parts_mut(result, 16);
    let digest = downcast_message::<Publish>(handle).digest;
    result_slice.copy_from_slice(&digest.to_be_bytes());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_publish_set_digest(
    handle: *mut MessageHandle,
    digest: *const u8,
) {
    let bytes = std::slice::from_raw_parts(digest, 16);
    let digest = u128::from_be_bytes(bytes.try_into().unwrap());
    downcast_message_mut::<Publish>(handle).digest = digest;
}

#[repr(C)]
pub struct HashRootPair {
    pub block_hash: [u8; 32],
    pub root: [u8; 32],
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_create(
    constants: *mut NetworkConstantsDto,
    block: *mut BlockHandle,
    roots_hashes: *const HashRootPair,
    roots_hashes_count: usize,
) -> *mut MessageHandle {
    create_message_handle(constants, |consts| {
        if !block.is_null() {
            let block = (*block).block.clone();
            ConfirmReq::with_block(consts, block)
        } else {
            let dtos = std::slice::from_raw_parts(roots_hashes, roots_hashes_count);
            let roots_hashes = dtos
                .iter()
                .map(|dto| {
                    (
                        BlockHash::from_bytes(dto.block_hash),
                        Root::from_bytes(dto.root),
                    )
                })
                .collect();
            ConfirmReq::with_roots_hashes(consts, roots_hashes)
        }
    })
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
pub unsafe extern "C" fn rsn_message_confirm_req_block(
    handle: *mut MessageHandle,
) -> *mut BlockHandle {
    match downcast_message::<ConfirmReq>(handle).block() {
        Some(block) => Box::into_raw(Box::new(BlockHandle::new(Arc::clone(block)))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_roots_hashes_count(
    handle: *mut MessageHandle,
) -> usize {
    downcast_message::<ConfirmReq>(handle).roots_hashes().len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_roots_hashes(
    handle: *mut MessageHandle,
    result: *mut HashRootPair,
) {
    let req = downcast_message::<ConfirmReq>(handle);
    let result_slice = std::slice::from_raw_parts_mut(result, req.roots_hashes().len());
    let mut i = 0;
    for (hash, root) in req.roots_hashes() {
        result_slice[i] = HashRootPair {
            block_hash: hash.to_bytes(),
            root: root.to_bytes(),
        };
        i += 1;
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_serialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
) -> bool {
    let mut stream = FfiStream::new(stream);
    downcast_message_mut::<ConfirmReq>(handle)
        .serialize(&mut stream)
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_deserialize(
    handle: *mut MessageHandle,
    stream: *mut c_void,
    uniquer: *mut BlockUniquerHandle,
) -> bool {
    let mut stream = FfiStream::new(stream);
    let uniquer = if uniquer.is_null() {
        None
    } else {
        Some((*uniquer).deref())
    };
    downcast_message_mut::<ConfirmReq>(handle)
        .deserialize(&mut stream, uniquer)
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_equals(
    handle_a: *mut MessageHandle,
    handle_b: *mut MessageHandle,
) -> bool {
    let a = downcast_message_mut::<ConfirmReq>(handle_a);
    let b = downcast_message_mut::<ConfirmReq>(handle_b);
    a == b
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_roots_string(
    handle: *mut MessageHandle,
    result: *mut StringDto,
) {
    let req = downcast_message_mut::<ConfirmReq>(handle);
    (*result) = req.roots_string().into();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_req_size(block_type: u8, count: usize) -> usize {
    ConfirmReq::serialized_size(BlockType::from_u8(block_type).unwrap(), count)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_confirm_ack_create(
    constants: *mut NetworkConstantsDto,
    vote: *mut VoteHandle,
) -> *mut MessageHandle {
    create_message_handle(constants, |consts| {
        let vote = (*vote).vote.clone();
        ConfirmAck::new(consts, vote)
    })
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
