use std::ffi::c_void;

use crate::{
    blocks::{ChangeBlock, ChangeHashables, LazyBlockHash},
    numbers::{Account, BlockHash, PublicKey, RawKey, Signature},
};

use crate::ffi::{
    property_tree::{FfiPropertyTreeReader, FfiPropertyTreeWriter},
    FfiStream,
};

pub struct ChangeBlockHandle {
    pub block: ChangeBlock,
}

#[repr(C)]
pub struct ChangeBlockDto {
    pub work: u64,
    pub signature: [u8; 64],
    pub previous: [u8; 32],
    pub representative: [u8; 32],
}

#[repr(C)]
pub struct ChangeBlockDto2 {
    pub previous: [u8; 32],
    pub representative: [u8; 32],
    pub priv_key: [u8; 32],
    pub pub_key: [u8; 32],
    pub work: u64,
}

#[no_mangle]
pub extern "C" fn rsn_change_block_create(dto: &ChangeBlockDto) -> *mut ChangeBlockHandle {
    Box::into_raw(Box::new(ChangeBlockHandle {
        block: ChangeBlock {
            work: dto.work,
            signature: Signature::from_bytes(dto.signature),
            hashables: ChangeHashables {
                previous: BlockHash::from_bytes(dto.previous),
                representative: Account::from_bytes(dto.representative),
            },
            hash: LazyBlockHash::new(),
            sideband: None,
        },
    }))
}

#[no_mangle]
pub extern "C" fn rsn_change_block_create2(dto: &ChangeBlockDto2) -> *mut ChangeBlockHandle {
    let block = match ChangeBlock::new(
        BlockHash::from_bytes(dto.previous),
        Account::from_bytes(dto.representative),
        &RawKey::from_bytes(dto.priv_key),
        &PublicKey::from_bytes(dto.pub_key),
        dto.work,
    ) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("could not create change block: {}", e);
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(ChangeBlockHandle { block }))
}

#[no_mangle]
pub extern "C" fn rsn_change_block_clone(handle: &ChangeBlockHandle) -> *mut ChangeBlockHandle {
    Box::into_raw(Box::new(ChangeBlockHandle {
        block: handle.block.clone(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_destroy(handle: *mut ChangeBlockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_work_set(handle: *mut ChangeBlockHandle, work: u64) {
    (*handle).block.work = work;
}

#[no_mangle]
pub extern "C" fn rsn_change_block_work(handle: &ChangeBlockHandle) -> u64 {
    handle.block.work
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_signature(
    handle: &ChangeBlockHandle,
    result: *mut [u8; 64],
) {
    (*result) = (*handle).block.signature.to_be_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_signature_set(
    handle: *mut ChangeBlockHandle,
    signature: &[u8; 64],
) {
    (*handle).block.signature = Signature::from_bytes(*signature);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_previous(
    handle: &ChangeBlockHandle,
    result: *mut [u8; 32],
) {
    (*result) = (*handle).block.hashables.previous.to_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_previous_set(
    handle: *mut ChangeBlockHandle,
    source: &[u8; 32],
) {
    (*handle).block.hashables.previous = BlockHash::from_bytes(*source);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_representative(
    handle: &ChangeBlockHandle,
    result: *mut [u8; 32],
) {
    (*result) = (*handle).block.hashables.representative.to_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_representative_set(
    handle: *mut ChangeBlockHandle,
    representative: &[u8; 32],
) {
    (*handle).block.hashables.representative = Account::from_bytes(*representative);
}

#[no_mangle]
pub extern "C" fn rsn_change_block_equals(a: &ChangeBlockHandle, b: &ChangeBlockHandle) -> bool {
    a.block.work.eq(&b.block.work)
        && a.block.signature.eq(&b.block.signature)
        && a.block.hashables.eq(&b.block.hashables)
}

#[no_mangle]
pub extern "C" fn rsn_change_block_size() -> usize {
    ChangeBlock::serialized_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_hash(handle: &ChangeBlockHandle, hash: *mut [u8; 32]) {
    (*hash) = handle.block.hash().to_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_serialize(
    handle: *mut ChangeBlockHandle,
    stream: *mut c_void,
) -> i32 {
    let mut stream = FfiStream::new(stream);
    if (*handle).block.serialize(&mut stream).is_ok() {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_change_block_deserialize(
    stream: *mut c_void,
) -> *mut ChangeBlockHandle {
    let mut stream = FfiStream::new(stream);
    match ChangeBlock::deserialize(&mut stream) {
        Ok(block) => Box::into_raw(Box::new(ChangeBlockHandle { block })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rsn_change_block_serialize_json(
    handle: &ChangeBlockHandle,
    ptree: *mut c_void,
) -> i32 {
    let mut writer = FfiPropertyTreeWriter::new(ptree);
    match handle.block.serialize_json(&mut writer) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn rsn_change_block_deserialize_json(
    ptree: *const c_void,
) -> *mut ChangeBlockHandle {
    let reader = FfiPropertyTreeReader::new(ptree);
    match ChangeBlock::deserialize_json(&reader) {
        Ok(block) => Box::into_raw(Box::new(ChangeBlockHandle { block })),
        Err(_) => std::ptr::null_mut(),
    }
}
