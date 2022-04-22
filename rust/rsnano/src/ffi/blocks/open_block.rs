use std::ffi::c_void;
use std::sync::{Arc, RwLock};

use crate::{
    Account, Block, BlockEnum, BlockHash, LazyBlockHash, OpenBlock, OpenHashables, PublicKey,
    RawKey, Signature,
};

use crate::ffi::{FfiPropertyTreeReader, FfiPropertyTreeWriter, FfiStream};

use super::BlockHandle;

#[repr(C)]
pub struct OpenBlockDto {
    pub work: u64,
    pub signature: [u8; 64],
    pub source: [u8; 32],
    pub representative: [u8; 32],
    pub account: [u8; 32],
}

#[repr(C)]
pub struct OpenBlockDto2 {
    pub source: [u8; 32],
    pub representative: [u8; 32],
    pub account: [u8; 32],
    pub priv_key: [u8; 32],
    pub pub_key: [u8; 32],
    pub work: u64,
}

#[no_mangle]
pub extern "C" fn rsn_open_block_create(dto: &OpenBlockDto) -> *mut BlockHandle {
    Box::into_raw(Box::new(BlockHandle {
        block: Arc::new(RwLock::new(BlockEnum::Open(OpenBlock {
            work: dto.work,
            signature: Signature::from_bytes(dto.signature),
            hashables: OpenHashables {
                source: BlockHash::from_bytes(dto.source),
                representative: Account::from_bytes(dto.representative),
                account: Account::from_bytes(dto.account),
            },
            hash: LazyBlockHash::new(),
            sideband: None,
        }))),
    }))
}

#[no_mangle]
pub extern "C" fn rsn_open_block_create2(dto: &OpenBlockDto2) -> *mut BlockHandle {
    let block = match OpenBlock::new(
        BlockHash::from_bytes(dto.source),
        Account::from_bytes(dto.representative),
        Account::from_bytes(dto.account),
        &RawKey::from_bytes(dto.priv_key),
        &PublicKey::from_bytes(dto.pub_key),
        dto.work,
    ) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("could not create open block: {}", e);
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(BlockHandle {
        block: Arc::new(RwLock::new(BlockEnum::Open(block))),
    }))
}

unsafe fn read_open_block<T>(handle: *const BlockHandle, f: impl FnOnce(&OpenBlock) -> T) -> T {
    let block = (*handle).block.read().unwrap();
    match &*block {
        BlockEnum::Open(b) => f(b),
        _ => panic!("expected open block"),
    }
}

unsafe fn write_open_block<T>(
    handle: *mut BlockHandle,
    mut f: impl FnMut(&mut OpenBlock) -> T,
) -> T {
    let mut block = (*handle).block.write().unwrap();
    match &mut *block {
        BlockEnum::Open(b) => f(b),
        _ => panic!("expected open block"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_work_set(handle: *mut BlockHandle, work: u64) {
    write_open_block(handle, |b| b.work = work);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_work(handle: *const BlockHandle) -> u64 {
    read_open_block(handle, |b| b.work)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_signature(handle: &BlockHandle, result: *mut [u8; 64]) {
    (*result) = read_open_block(handle, |b| b.signature.to_be_bytes());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_signature_set(
    handle: *mut BlockHandle,
    signature: &[u8; 64],
) {
    write_open_block(handle, |b| b.signature = Signature::from_bytes(*signature));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_source(handle: *const BlockHandle, result: *mut [u8; 32]) {
    (*result) = read_open_block(handle, |b| b.hashables.source.to_bytes());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_source_set(handle: *mut BlockHandle, source: &[u8; 32]) {
    write_open_block(handle, |b| {
        b.hashables.source = BlockHash::from_bytes(*source)
    });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_representative(
    handle: *const BlockHandle,
    result: *mut [u8; 32],
) {
    (*result) = read_open_block(handle, |b| b.hashables.representative.to_bytes());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_representative_set(
    handle: *mut BlockHandle,
    representative: &[u8; 32],
) {
    write_open_block(handle, |b| {
        b.hashables.representative = Account::from_bytes(*representative)
    });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_account(handle: *const BlockHandle, result: *mut [u8; 32]) {
    (*result) = read_open_block(handle, |b| b.hashables.account.to_bytes());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_account_set(handle: *mut BlockHandle, account: &[u8; 32]) {
    write_open_block(handle, |b| {
        b.hashables.account = Account::from_bytes(*account)
    });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_equals(
    a: *const BlockHandle,
    b: *const BlockHandle,
) -> bool {
    let a_guard = (*a).block.read().unwrap();
    let b_guard = (*b).block.read().unwrap();
    if let BlockEnum::Open(a_block) = &*a_guard {
        if let BlockEnum::Open(b_block) = &*b_guard {
            return a_block.work.eq(&b_block.work)
                && a_block.signature.eq(&b_block.signature)
                && a_block.hashables.eq(&b_block.hashables);
        }
    }
    false
}

#[no_mangle]
pub extern "C" fn rsn_open_block_size() -> usize {
    OpenBlock::serialized_size()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_hash(handle: *const BlockHandle, hash: *mut [u8; 32]) {
    (*hash) = read_open_block(handle, |b| b.hash().to_bytes());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_serialize(
    handle: *mut BlockHandle,
    stream: *mut c_void,
) -> i32 {
    let mut stream = FfiStream::new(stream);
    read_open_block(handle, |b| {
        if b.serialize(&mut stream).is_ok() {
            0
        } else {
            -1
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_deserialize(stream: *mut c_void) -> *mut BlockHandle {
    let mut stream = FfiStream::new(stream);
    match OpenBlock::deserialize(&mut stream) {
        Ok(block) => Box::into_raw(Box::new(BlockHandle {
            block: Arc::new(RwLock::new(BlockEnum::Open(block))),
        })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_open_block_serialize_json(
    handle: *const BlockHandle,
    ptree: *mut c_void,
) -> i32 {
    let mut writer = FfiPropertyTreeWriter::new(ptree);
    read_open_block(handle, |b| match b.serialize_json(&mut writer) {
        Ok(_) => 0,
        Err(_) => -1,
    })
}

#[no_mangle]
pub extern "C" fn rsn_open_block_deserialize_json(ptree: *const c_void) -> *mut BlockHandle {
    let reader = FfiPropertyTreeReader::new(ptree);
    match OpenBlock::deserialize_json(&reader) {
        Ok(block) => Box::into_raw(Box::new(BlockHandle {
            block: Arc::new(RwLock::new(BlockEnum::Open(block))),
        })),
        Err(_) => std::ptr::null_mut(),
    }
}
