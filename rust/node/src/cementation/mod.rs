mod accounts_confirmed_map;
mod batch_write_size_manager;
mod block_cache;
mod block_cementer;
mod block_cementer_logic;
mod block_queue;
mod cementation_queue;
mod cementation_thread;
mod cementation_walker;
mod confirming_set;
mod ledger_data_requester;
mod write_batcher;

use std::sync::Arc;

use block_queue::BlockQueue;
use rsnano_core::{BlockChainSection, BlockEnum, BlockHash};

use accounts_confirmed_map::{
    AccountsConfirmedMap, AccountsConfirmedMapContainerInfo, ConfirmedInfo,
};
use batch_write_size_manager::BatchWriteSizeManager;
pub use block_cementer::BlockCementer;
use ledger_data_requester::{LedgerAdapter, LedgerDataRequester};

#[cfg(test)]
use ledger_data_requester::LedgerDataRequesterStub;

use block_cache::BlockCache;
use block_cementer_logic::{
    BlockCementerContainerInfo, BlockCementerLogic, BlockCementerLogicOptions, FlushDecision,
};
use cementation_queue::{CementationQueue, CementationQueueContainerInfo};
pub use cementation_thread::{CementCallbackRefs, CementCallbacks, CementationThread};
use cementation_walker::CementationWalker;
pub use confirming_set::*;
use write_batcher::{WriteBatcher, WriteBatcherOptions};

/// We need these details whenever we want to write the new
/// confirmation height to the ledger
#[derive(Clone, Debug)]
pub struct ConfHeightDetails {
    pub update_height: BlockChainSection,
    /// is this a list of cemented blocks in descending order?
    pub cemented_in_current_account: Vec<BlockHash>,
    /// is this a list of cemented blocks that belong to another account that we received from?
    pub cemented_in_source: Vec<BlockHash>,
}

type BlockCallback = Box<dyn FnMut(&Arc<BlockEnum>) + Send>;
type BlockHashCallback = Box<dyn FnMut(BlockHash) + Send>;
type AwaitingProcessingCountCallback = Box<dyn FnMut() -> u64 + Send>;
