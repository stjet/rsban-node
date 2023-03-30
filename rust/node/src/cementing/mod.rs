mod automatic_mode;
mod block_cache;
mod block_cementor;
mod block_queue;
mod bounded_mode;
mod cement_queue;
mod confirmation_height_processor;
mod confirmed_iterated_pairs;
mod implicit_receive_cemented_mapping;
mod unbounded_mode;
mod unconfirmed_receive_and_sources_collector;

use std::sync::Arc;

use block_queue::BlockQueue;
use rsnano_core::{BlockEnum, BlockHash, UpdateConfirmationHeight};

pub use automatic_mode::ConfirmationHeightMode;
use automatic_mode::{AutomaticMode, AutomaticModeContainerInfo, UNBOUNDED_CUTOFF};
use bounded_mode::{BoundedMode, BoundedModeContainerInfo};
use confirmation_height_processor::CementCallbackRefs;
pub use confirmation_height_processor::ConfirmationHeightProcessor;
use unbounded_mode::{UnboundedMode, UnboundedModeContainerInfo};

/// We need these details whenever we want to write the new
/// confirmation height to the ledger
#[derive(Clone, Debug)]
pub struct ConfHeightDetails {
    pub update_height: UpdateConfirmationHeight,
    /// is this a list of cemented blocks in descending order?
    pub cemented_in_current_account: Vec<BlockHash>,
    /// is this a list of cemented blocks that belong to another account that we received from?
    pub cemented_in_source: Vec<BlockHash>,
}

type BlockCallback = Box<dyn FnMut(&Arc<BlockEnum>) + Send>;
type BlockHashCallback = Box<dyn FnMut(BlockHash) + Send>;
type AwaitingProcessingCountCallback = Box<dyn FnMut() -> u64 + Send>;
