mod block_details;
mod change_block;
mod open_block;
mod receive_block;
mod send_block;
mod state_block;

use std::{convert::TryFrom, ffi::c_void};

pub use block_details::*;
pub use change_block::*;
pub use open_block::*;
pub use receive_block::*;
pub use send_block::*;
pub use state_block::*;

use super::{property_tree::FfiPropertyTreeReader, FfiStream};
use crate::blocks::{
    deserialize_block_json, serialized_block_size, Block, BlockEnum, BlockSideband, BlockType,
};
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
    block: &BlockDto,
    sideband: *mut BlockSidebandDto,
) -> i32 {
    match as_block(block) {
        Some(block) => match block.sideband() {
            Some(sb) => {
                set_block_sideband_dto(sb, sideband);
                0
            }
            None => 0, // test confirmation_heightDeathTest.rollback_added_block calls sideband() event though its None ?!
        },
        None => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_sideband_set(
    block: *mut BlockDto,
    sideband: &BlockSidebandDto,
) -> i32 {
    match as_block_mut(block.as_mut()) {
        Some(block) => match BlockSideband::try_from(sideband) {
            Ok(sideband) => {
                block.set_sideband(sideband);
                0
            }
            Err(_) => -1,
        },
        None => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_block_has_sideband(block: &BlockDto) -> bool {
    if let Some(block) = as_block(block) {
        block.sideband().is_some()
    } else {
        false
    }
}

unsafe fn as_block_mut(dto: Option<&mut BlockDto>) -> Option<&mut dyn Block> {
    let dto = match dto {
        Some(x) => x,
        None => return None,
    };

    let block_type: BlockType = match FromPrimitive::from_u8(dto.block_type) {
        Some(t) => t,
        None => return None,
    };

    match block_type {
        BlockType::Invalid | BlockType::NotABlock => None,
        BlockType::Send => (dto.handle as *mut SendBlockHandle)
            .as_mut()
            .map(|x| &mut x.block as &mut dyn Block),
        BlockType::Receive => (dto.handle as *mut ReceiveBlockHandle)
            .as_mut()
            .map(|x| &mut x.block as &mut dyn Block),
        BlockType::Open => (dto.handle as *mut OpenBlockHandle)
            .as_mut()
            .map(|x| &mut x.block as &mut dyn Block),
        BlockType::Change => (dto.handle as *mut ChangeBlockHandle)
            .as_mut()
            .map(|x| &mut x.block as &mut dyn Block),
        BlockType::State => (dto.handle as *mut StateBlockHandle)
            .as_mut()
            .map(|x| &mut x.block as &mut dyn Block),
    }
}

unsafe fn as_block(dto: &BlockDto) -> Option<&dyn Block> {
    let block_type: BlockType = match FromPrimitive::from_u8(dto.block_type) {
        Some(t) => t,
        None => return None,
    };

    match block_type {
        BlockType::Invalid | BlockType::NotABlock => None,
        BlockType::Send => (dto.handle as *const SendBlockHandle)
            .as_ref()
            .map(|x| &x.block as &dyn Block),
        BlockType::Receive => (dto.handle as *const ReceiveBlockHandle)
            .as_ref()
            .map(|x| &x.block as &dyn Block),
        BlockType::Open => (dto.handle as *const OpenBlockHandle)
            .as_ref()
            .map(|x| &x.block as &dyn Block),
        BlockType::Change => (dto.handle as *const ChangeBlockHandle)
            .as_ref()
            .map(|x| &x.block as &dyn Block),
        BlockType::State => (dto.handle as *const StateBlockHandle)
            .as_ref()
            .map(|x| &x.block as &dyn Block),
    }
}

#[repr(C)]
pub struct BlockDto {
    pub block_type: u8,
    pub handle: *mut c_void,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_deserialize_block_json(
    dto: *mut BlockDto,
    ptree: *const c_void,
) -> i32 {
    let ptree_reader = FfiPropertyTreeReader::new(ptree);
    match deserialize_block_json(&ptree_reader) {
        Ok(block) => {
            set_block_dto(&mut (*dto), block);
            0
        }
        Err(_) => -1,
    }
}

pub fn set_block_dto(dto: &mut BlockDto, block: BlockEnum){
    dto.block_type = block.block_type() as u8;
    dto.handle = match block {
        BlockEnum::Send(block) => {
            Box::into_raw(Box::new(SendBlockHandle { block })) as *mut c_void
        }
        BlockEnum::Receive(block) => {
            Box::into_raw(Box::new(ReceiveBlockHandle { block })) as *mut c_void
        }
        BlockEnum::Open(block) => {
            Box::into_raw(Box::new(OpenBlockHandle { block })) as *mut c_void
        }
        BlockEnum::Change(block) => {
            Box::into_raw(Box::new(ChangeBlockHandle { block })) as *mut c_void
        }
        BlockEnum::State(block) => {
            Box::into_raw(Box::new(StateBlockHandle { block })) as *mut c_void
        }
    };
}