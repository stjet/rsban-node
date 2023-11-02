use rsnano_core::{Account, BlockHash};

use super::{
    create_message_handle3, downcast_message, downcast_message_mut, message_handle_clone,
    MessageHandle, MessageHeaderHandle,
};
use crate::{
    core::{copy_block_array_dto, BlockArrayDto, BlockHandle},
    NetworkConstantsDto,
};
use rsnano_node::messages::{
    AccountInfoAckPayload, AscPullAckPayload, AscPullAckType, MessageEnum, Payload,
};
use std::{borrow::Borrow, ops::Deref, sync::Arc};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_ack_create2(
    constants: *mut NetworkConstantsDto,
    id: u64,
    payload: *const AccountInfoAckPayloadDto,
) -> *mut MessageHandle {
    let payload = (*payload).borrow().into();
    create_message_handle3(constants, move |protocol_info| {
        MessageEnum::new_asc_pull_ack_accounts(protocol_info, id, payload)
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_ack_create3(
    constants: *mut NetworkConstantsDto,
    id: u64,
    blocks: *const *const BlockHandle,
    count: usize,
) -> *mut MessageHandle {
    let blocks = std::slice::from_raw_parts(blocks, count);
    let blocks = blocks
        .iter()
        .map(|&b| (*b).deref().deref().clone())
        .collect();

    create_message_handle3(constants, move |protocol_info| {
        MessageEnum::new_asc_pull_ack_blocks(protocol_info, id, blocks)
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_ack_clone(
    handle: *mut MessageHandle,
) -> *mut MessageHandle {
    message_handle_clone::<MessageEnum>(handle)
}

unsafe fn get_payload_mut(handle: *mut MessageHandle) -> &'static mut AscPullAckPayload {
    let msg = downcast_message_mut::<MessageEnum>(handle);
    let Payload::AscPullAck(payload) = &mut msg.payload else { panic!("not an asc_pull_ack")};
    payload
}

unsafe fn get_payload(handle: *mut MessageHandle) -> &'static AscPullAckPayload {
    let msg = downcast_message::<MessageEnum>(handle);
    let Payload::AscPullAck(payload) = &msg.payload else { panic!("not an asc_pull_ack")};
    payload
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_ack_set_id(handle: *mut MessageHandle, id: u64) {
    get_payload_mut(handle).id = id;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_ack_get_id(handle: *mut MessageHandle) -> u64 {
    get_payload(handle).id
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_ack_pull_type(handle: *mut MessageHandle) -> u8 {
    get_payload(handle).payload_type() as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_ack_size(header: *mut MessageHeaderHandle) -> usize {
    AscPullAckPayload::serialized_size(&*header)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_ack_payload_blocks(
    handle: *mut MessageHandle,
    blocks: &mut BlockArrayDto,
) {
    match &get_payload(handle).pull_type {
        AscPullAckType::Blocks(blks) => {
            let list = blks.blocks().iter().map(|b| Arc::new(b.clone())).collect();
            copy_block_array_dto(list, blocks)
        }
        _ => panic!("not a blocks payload"),
    }
}

#[repr(C)]
pub struct AccountInfoAckPayloadDto {
    pub account: [u8; 32],
    pub account_open: [u8; 32],
    pub account_head: [u8; 32],
    pub account_block_count: u64,
    pub account_conf_frontier: [u8; 32],
    pub account_conf_height: u64,
}

impl From<&AccountInfoAckPayload> for AccountInfoAckPayloadDto {
    fn from(payload: &AccountInfoAckPayload) -> Self {
        Self {
            account: *payload.account.as_bytes(),
            account_open: *payload.account_open.as_bytes(),
            account_head: *payload.account_head.as_bytes(),
            account_block_count: payload.account_block_count,
            account_conf_frontier: *payload.account_conf_frontier.as_bytes(),
            account_conf_height: payload.account_conf_height,
        }
    }
}

impl From<&AccountInfoAckPayloadDto> for AccountInfoAckPayload {
    fn from(dto: &AccountInfoAckPayloadDto) -> Self {
        Self {
            account: Account::from_bytes(dto.account),
            account_open: BlockHash::from_bytes(dto.account_open),
            account_head: BlockHash::from_bytes(dto.account_head),
            account_block_count: dto.account_block_count,
            account_conf_frontier: BlockHash::from_bytes(dto.account_conf_frontier),
            account_conf_height: dto.account_conf_height,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_asc_pull_ack_payload_account_info(
    handle: *mut MessageHandle,
    result: *mut AccountInfoAckPayloadDto,
) {
    match &get_payload(handle).pull_type {
        AscPullAckType::AccountInfo(account_info) => (*result) = account_info.into(),
        _ => panic!("not an account_info payload"),
    }
}
