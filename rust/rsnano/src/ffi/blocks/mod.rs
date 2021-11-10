mod block_details;
mod change_block;
mod open_block;
mod receive_block;
mod send_block;
mod state_block;

use std::ffi::c_void;

pub use block_details::*;
pub use change_block::*;
pub use open_block::*;
pub use receive_block::*;
pub use send_block::*;
pub use state_block::*;

use super::{property_tree::FfiPropertyTreeReader, FfiStream};
use crate::blocks::{deserialize_block_json, serialized_block_size, Block};
use num::FromPrimitive;

#[no_mangle]
pub extern "C" fn rsn_block_serialized_size(block_type: u8) -> usize {
    match FromPrimitive::from_u8(block_type) {
        Some(block_type) => serialized_block_size(block_type),
        None => 0,
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
            (*dto).block_type = block.block_type() as u8;
            (*dto).handle = match block {
                Block::Send(block) => {
                    Box::into_raw(Box::new(SendBlockHandle { block })) as *mut c_void
                }
                Block::Receive(block) => {
                    Box::into_raw(Box::new(ReceiveBlockHandle { block })) as *mut c_void
                }
                Block::Open(block) => {
                    Box::into_raw(Box::new(OpenBlockHandle { block })) as *mut c_void
                }
                Block::Change(block) => {
                    Box::into_raw(Box::new(ChangeBlockHandle { block })) as *mut c_void
                }
                Block::State(block) => {
                    Box::into_raw(Box::new(StateBlockHandle { block })) as *mut c_void
                }
            };
            0
        }
        Err(_) => -1,
    }
}
