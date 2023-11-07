mod block_details;
mod block_uniquer;
mod change_block;
mod open_block;
mod receive_block;
mod send_block;
mod state_block;

use std::{
    convert::TryFrom,
    ffi::c_void,
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub use block_details::*;
pub use block_uniquer::BlockUniquerHandle;
pub use change_block::*;
pub use open_block::*;
pub use receive_block::*;
use rsnano_core::{
    deserialize_block_json, serialized_block_size, BlockEnum, BlockSideband, BlockType, Signature,
};
pub use send_block::*;
pub use state_block::*;

use crate::{utils::FfiStream, FfiPropertyTreeReader, FfiPropertyTreeWriter};
use num::FromPrimitive;
use rsnano_node::utils::deserialize_block;

mod block_vec;
pub use block_vec::BlockVecHandle;

#[no_mangle]
pub extern "C" fn rsn_block_serialized_size(block_type: u8) -> usize {
    match FromPrimitive::from_u8(block_type) {
        Some(block_type) => serialized_block_size(block_type),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn rsn_block_sideband(block: &BlockHandle, sideband: &mut BlockSidebandDto) -> i32 {
    let b = block.deref();
    match b.sideband() {
        Some(sb) => {
            set_block_sideband_dto(sb, sideband);
            0
        }
        None => -1,
    }
}

#[no_mangle]
pub extern "C" fn rsn_block_clone(handle: &BlockHandle) -> *mut BlockHandle {
    let cloned = handle.deref().deref().clone();
    Box::into_raw(Box::new(BlockHandle(Arc::new(cloned))))
}

#[no_mangle]
pub extern "C" fn rsn_block_handle_clone(handle: &BlockHandle) -> *mut BlockHandle {
    Box::into_raw(Box::new(BlockHandle(Arc::clone(handle.deref()))))
}

#[no_mangle]
pub extern "C" fn rsn_block_rust_data_pointer(handle: &BlockHandle) -> *const c_void {
    let ptr = Arc::as_ptr(handle.deref());
    ptr as *const c_void
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_destroy(handle: *mut BlockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_block_sideband_set(
    block: &mut BlockHandle,
    sideband: &BlockSidebandDto,
) -> i32 {
    match BlockSideband::try_from(sideband) {
        Ok(sideband) => {
            block.get_mut().as_block_mut().set_sideband(sideband);
            0
        }
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_has_sideband(block: &BlockHandle) -> bool {
    block.deref().sideband().is_some()
}

pub struct BlockHandle(pub Arc<BlockEnum>);

impl BlockHandle {
    pub fn new(block: Arc<BlockEnum>) -> *mut Self {
        Box::into_raw(Box::new(Self(block)))
    }
    pub fn get_mut(&mut self) -> &mut BlockEnum {
        Arc::get_mut(&mut self.0).expect("Could not make block mutable")
    }
}

impl Deref for BlockHandle {
    type Target = Arc<BlockEnum>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BlockHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_deserialize_block_json(ptree: *const c_void) -> *mut BlockHandle {
    let ptree_reader = FfiPropertyTreeReader::new(ptree);
    match deserialize_block_json(&ptree_reader) {
        Ok(block) => Box::into_raw(Box::new(BlockHandle(Arc::new(block)))),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rsn_block_type(handle: &BlockHandle) -> u8 {
    handle.deref().block_type() as u8
}

#[no_mangle]
pub extern "C" fn rsn_block_work_set(handle: &mut BlockHandle, work: u64) {
    handle.get_mut().as_block_mut().set_work(work);
}

#[no_mangle]
pub extern "C" fn rsn_block_work(handle: &BlockHandle) -> u64 {
    handle.deref().work()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_signature(handle: &BlockHandle, result: *mut [u8; 64]) {
    (*result) = *handle.deref().block_signature().as_bytes();
}

#[no_mangle]
pub extern "C" fn rsn_block_signature_set(handle: &mut BlockHandle, signature: &[u8; 64]) {
    handle
        .get_mut()
        .set_block_signature(&Signature::from_bytes(*signature));
}

#[no_mangle]
pub extern "C" fn rsn_block_previous(handle: &BlockHandle, result: &mut [u8; 32]) {
    *result = *handle.deref().previous().as_bytes();
}

#[no_mangle]
pub extern "C" fn rsn_block_equals(a: &BlockHandle, b: &BlockHandle) -> bool {
    a.deref().deref().eq(b.deref().deref())
}

#[no_mangle]
pub extern "C" fn rsn_block_hash(handle: &BlockHandle, hash: &mut [u8; 32]) {
    *hash = *handle.deref().hash().as_bytes();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_full_hash(handle: &BlockHandle, hash: *mut u8) {
    let result = std::slice::from_raw_parts_mut(hash, 32);
    let hash = handle.deref().full_hash();
    result.copy_from_slice(hash.as_bytes());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_serialize(handle: &BlockHandle, stream: *mut c_void) -> i32 {
    let mut stream = FfiStream::new(stream);
    handle.deref().serialize_safe(&mut stream);
    0
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_serialize_json(handle: &BlockHandle, ptree: *mut c_void) -> i32 {
    let mut writer = FfiPropertyTreeWriter::new_borrowed(ptree);
    match handle.deref().serialize_json(&mut writer) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_deserialize_block(
    block_type: u8,
    stream: *mut c_void,
    uniquer: *mut BlockUniquerHandle,
) -> *mut BlockHandle {
    let mut stream = FfiStream::new(stream);
    let block_type = match BlockType::from_u8(block_type) {
        Some(bt) => bt,
        None => return std::ptr::null_mut(),
    };

    let uniquer = if uniquer.is_null() {
        None
    } else {
        Some((*uniquer).deref().as_ref())
    };

    match deserialize_block(block_type, &mut stream, uniquer) {
        Ok(block) => Box::into_raw(Box::new(BlockHandle(block))),
        Err(_) => std::ptr::null_mut(),
    }
}

pub struct BlockArrayRawPtr(Vec<*mut BlockHandle>);

pub(crate) fn copy_block_array_dto(blocks: Vec<Arc<BlockEnum>>, target: &mut BlockArrayDto) {
    let mut raw_block_array = Box::new(BlockArrayRawPtr(Vec::new()));
    for block in blocks {
        raw_block_array
            .0
            .push(Box::into_raw(Box::new(BlockHandle(block))));
    }
    target.blocks = raw_block_array.0.as_ptr();
    target.count = raw_block_array.0.len();
    target.raw_ptr = Box::into_raw(raw_block_array);
}

#[repr(C)]
pub struct BlockArrayDto {
    pub blocks: *const *mut BlockHandle,
    pub count: usize,
    pub raw_ptr: *mut BlockArrayRawPtr,
}

impl BlockArrayDto {
    pub unsafe fn blocks(&self) -> impl Iterator<Item = &Arc<BlockEnum>> {
        let dtos = std::slice::from_raw_parts(self.blocks, self.count);
        dtos.iter().map(|&b| (*b).deref())
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_array_destroy(dto: *mut BlockArrayDto) {
    drop(Box::from_raw((*dto).raw_ptr))
}

impl From<Vec<BlockEnum>> for BlockArrayDto {
    fn from(value: Vec<BlockEnum>) -> Self {
        let mut raw_block_array = Box::new(BlockArrayRawPtr(Vec::new()));
        for block in value {
            raw_block_array
                .0
                .push(Box::into_raw(Box::new(BlockHandle(Arc::new(block)))));
        }

        Self {
            blocks: raw_block_array.0.as_ptr(),
            count: raw_block_array.0.len(),
            raw_ptr: Box::into_raw(raw_block_array),
        }
    }
}

pub type BlockCallback = extern "C" fn(*mut c_void, *mut BlockHandle);
pub type BlockHashCallback = extern "C" fn(*mut c_void, *const u8);
