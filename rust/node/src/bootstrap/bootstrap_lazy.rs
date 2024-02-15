use crate::{block_processing::BlockProcessor, websocket::Listener};
use anyhow::Result;
use rsnano_core::{Account, BlockEnum};
use rsnano_ledger::Ledger;
use std::{
    ffi::c_void,
    sync::{Arc, Weak},
};

use super::{BootstrapAttempt, BootstrapInitiator, BootstrapMode};

pub static mut LAZY_PROCESS_BLOCK: Option<
    fn(*mut c_void, Arc<BlockEnum>, &Account, u64, u32, bool, u32) -> bool,
> = None;

pub struct BootstrapAttemptLazy {
    pub attempt: BootstrapAttempt,
    cpp_handle: *mut c_void,
}

unsafe impl Send for BootstrapAttemptLazy {}
unsafe impl Sync for BootstrapAttemptLazy {}

impl BootstrapAttemptLazy {
    pub fn new(
        cpp_handle: *mut c_void,
        websocket_server: Arc<dyn Listener>,
        block_processor: Weak<BlockProcessor>,
        bootstrap_initiator: Weak<BootstrapInitiator>,
        ledger: Arc<Ledger>,
        id: &str,
        incremental_id: u64,
    ) -> Result<Self> {
        Ok(Self {
            cpp_handle,
            attempt: BootstrapAttempt::new(
                websocket_server,
                block_processor,
                bootstrap_initiator,
                ledger,
                id,
                BootstrapMode::Lazy,
                incremental_id,
            )?,
        })
    }

    pub fn process_block(
        &self,
        block: Arc<BlockEnum>,
        known_account: &Account,
        pull_blocks_processed: u64,
        max_blocks: u32,
        block_expected: bool,
        retry_limit: u32,
    ) -> bool {
        unsafe {
            LAZY_PROCESS_BLOCK.unwrap()(
                self.cpp_handle,
                block,
                known_account,
                pull_blocks_processed,
                max_blocks,
                block_expected,
                retry_limit,
            )
        }
    }
}
