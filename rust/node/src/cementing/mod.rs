use rsnano_core::{Account, BlockHash};

mod block_cache;
mod block_cementor;
mod cement_queue;
mod confirmation_height_bounded;
mod confirmation_height_unbounded;
mod confirmed_iterated_pairs;
mod implicit_receive_cemented_mapping;
mod unconfirmed_receive_and_sources_collector;

pub use confirmation_height_bounded::{truncate_after, ConfirmationHeightBounded};
pub use confirmation_height_unbounded::ConfirmationHeightUnbounded;
pub use confirmed_iterated_pairs::ConfirmedIteratedPair;

/// We need these details whenever we want to write the new
/// confirmation height to the ledger
#[derive(Clone, Debug)]
pub struct ConfHeightDetails {
    pub account: Account,
    pub latest_confirmed_block: BlockHash,
    pub new_height: u64,
    pub num_blocks_confirmed: u64,
    /// is this a list of cemented blocks in descending order?
    pub cemented_in_current_account: Vec<BlockHash>,
    /// is this a list of cemented blocks that belong to another account that we received from?
    pub cemented_in_source: Vec<BlockHash>,
}
