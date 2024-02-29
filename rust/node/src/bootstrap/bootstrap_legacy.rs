use super::{BootstrapAttempt, BootstrapInitiator, BootstrapMode, PullInfo};
use crate::{block_processing::BlockProcessor, websocket::Listener};
use rsnano_core::{Account, BlockHash};
use rsnano_ledger::Ledger;
use std::{
    ffi::c_void,
    sync::{Arc, Weak},
};

pub struct BootstrapAttemptLegacy {
    cpp_handle: *mut c_void,
    pub attempt: BootstrapAttempt,
}

impl BootstrapAttemptLegacy {
    pub fn new(
        cpp_handle: *mut c_void,
        websocket_server: Arc<dyn Listener>,
        block_processor: Weak<BlockProcessor>,
        bootstrap_initiator: Weak<BootstrapInitiator>,
        ledger: Arc<Ledger>,
        id: &str,
        incremental_id: u64,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            cpp_handle,
            attempt: BootstrapAttempt::new(
                websocket_server,
                block_processor,
                bootstrap_initiator,
                ledger,
                id,
                BootstrapMode::Legacy,
                incremental_id,
            )?,
        })
    }

    pub fn request_bulk_push_target(&self) -> Option<(BlockHash, BlockHash)> {
        unsafe {
            REQUEST_BULK_PUSH_TARGET.expect("REQUEST_BULK_PUSH_TARGET missing")(self.cpp_handle)
        }
    }

    pub fn add_frontier(&self, pull_info: &PullInfo) {
        unsafe {
            ADD_FRONTIER.expect("ADD_FRONTIER missing")(self.cpp_handle, pull_info);
        }
    }

    pub fn set_start_account(&self, account: Account) {
        unsafe {
            ADD_START_ACCOUNT.expect("ADD_START_ACCOUNT missing")(self.cpp_handle, account);
        }
    }

    pub fn add_bulk_push_target(&self, head: &BlockHash, end: &BlockHash) {
        unsafe {
            ADD_BULK_PUSH_TARGET.expect("ADD_BULK_PUSH_TARGET missing")(self.cpp_handle, head, end);
        }
    }
}

unsafe impl Send for BootstrapAttemptLegacy {}
unsafe impl Sync for BootstrapAttemptLegacy {}

pub static mut ADD_FRONTIER: Option<fn(*mut c_void, &PullInfo)> = None;
pub static mut ADD_START_ACCOUNT: Option<fn(*mut c_void, Account)> = None;
pub static mut ADD_BULK_PUSH_TARGET: Option<fn(*mut c_void, &BlockHash, &BlockHash)> = None;
pub static mut REQUEST_BULK_PUSH_TARGET: Option<fn(*mut c_void) -> Option<(BlockHash, BlockHash)>> =
    None;
