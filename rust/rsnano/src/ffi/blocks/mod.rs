mod block_details;
mod change_block;
mod open_block;
mod receive_block;
mod send_block;
mod state_block;

use std::{
    convert::TryFrom,
    ffi::c_void,
    sync::{Arc, RwLock},
};

pub use block_details::*;
pub use change_block::*;
pub use open_block::*;
pub use receive_block::*;
pub use send_block::*;
pub use state_block::*;

use super::{property_tree::FfiPropertyTreeReader, FfiStream};
use crate::blocks::{deserialize_block_json, serialized_block_size, BlockEnum, BlockSideband};
use num::FromPrimitive;

#[no_mangle]
pub extern "C" fn rsn_block_serialized_size(block_type: u8) -> usize {
    match FromPrimitive::from_u8(block_type) {
        Some(block_type) => serialized_block_size(block_type),
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_sideband(
    block: *const BlockHandle,
    sideband: *mut BlockSidebandDto,
) -> i32 {
    let b = (*block).block.read().unwrap();
    let block = (&*b).as_block();
    match block.sideband() {
        Some(sb) => {
            set_block_sideband_dto(sb, sideband);
            0
        }
        None => 0, // test confirmation_heightDeathTest.rollback_added_block calls sideband() event though its None ?!
    }
}

#[no_mangle]
pub extern "C" fn rsn_block_clone(handle: &BlockHandle) -> *mut BlockHandle {
    let cloned = handle.block.read().unwrap().clone();
    Box::into_raw(Box::new(BlockHandle {
        block: Arc::new(RwLock::new(cloned)),
    }))
}

#[no_mangle]
pub extern "C" fn rsn_block_handle_clone(handle: &BlockHandle) -> *mut BlockHandle {
    Box::into_raw(Box::new(BlockHandle {
        block: handle.block.clone(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_destroy(handle: *mut BlockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_sideband_set(
    block: *mut BlockHandle,
    sideband: &BlockSidebandDto,
) -> i32 {
    match BlockSideband::try_from(sideband) {
        Ok(sideband) => {
            (&mut *block)
                .block
                .write()
                .unwrap()
                .as_block_mut()
                .set_sideband(sideband);
            0
        }
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_has_sideband(block: *const BlockHandle) -> bool {
    let block = (*block).block.read().unwrap();
    block.as_block().sideband().is_some()
}

pub struct BlockHandle {
    pub(crate) block: Arc<RwLock<BlockEnum>>,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_shared_block_enum_handle_destroy(handle: *mut BlockHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_deserialize_block_json(ptree: *const c_void) -> *mut BlockHandle {
    let ptree_reader = FfiPropertyTreeReader::new(ptree);
    match deserialize_block_json(&ptree_reader) {
        Ok(block) => Box::into_raw(Box::new(BlockHandle {
            block: Arc::new(RwLock::new(block)),
        })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_type(handle: *const BlockHandle) -> u8 {
    (*handle).block.read().unwrap().as_block().block_type() as u8
}
