use rsnano_core::{Account, BlockHash};

mod block_cementor;
mod cement_queue;
mod confirmation_height_unbounded;
mod confirmed_iterated_pairs;
mod implicit_receive_cemented_mapping;

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
    pub block_callback_data: Vec<BlockHash>,
    pub source_block_callback_data: Vec<BlockHash>,
}
