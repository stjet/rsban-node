mod accounts_confirmed_map;
mod batch_write_size_manager;
mod block_cache;
mod block_queue;
mod block_cementer;
mod bounded_mode_helper;
mod cementation_thread;
mod ledger_data_requester;
mod multi_account_cementer;
mod single_account_cementer;
mod write_details_queue;

use std::sync::Arc;

use block_queue::BlockQueue;
use rsnano_core::{BlockEnum, BlockHash, ConfirmationHeightUpdate};

use accounts_confirmed_map::{
    AccountsConfirmedMap, AccountsConfirmedMapContainerInfo, ConfirmedInfo,
};
use batch_write_size_manager::BatchWriteSizeManager;
use block_cementer::{BlockCementer, BoundedModeContainerInfo};
use ledger_data_requester::{LedgerAdapter, LedgerDataRequester};

#[cfg(test)]
use ledger_data_requester::LedgerDataRequesterStub;

use block_cache::BlockCache;
use cementation_thread::CementCallbackRefs;
pub use cementation_thread::CementationThread;
use multi_account_cementer::MultiAccountCementer;
use single_account_cementer::SingleAccountCementer;
use write_details_queue::{WriteDetails, WriteDetailsContainerInfo, WriteDetailsQueue};

/// We need these details whenever we want to write the new
/// confirmation height to the ledger
#[derive(Clone, Debug)]
pub struct ConfHeightDetails {
    pub update_height: ConfirmationHeightUpdate,
    /// is this a list of cemented blocks in descending order?
    pub cemented_in_current_account: Vec<BlockHash>,
    /// is this a list of cemented blocks that belong to another account that we received from?
    pub cemented_in_source: Vec<BlockHash>,
}

type BlockCallback = Box<dyn FnMut(&Arc<BlockEnum>) + Send>;
type BlockHashCallback = Box<dyn FnMut(BlockHash) + Send>;
type AwaitingProcessingCountCallback = Box<dyn FnMut() -> u64 + Send>;
