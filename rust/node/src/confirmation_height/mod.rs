use rsnano_core::{Account, BlockHash};

pub struct ConfirmationHeightUnbounded {}

impl ConfirmationHeightUnbounded {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Clone)]
pub struct ConfHeightDetails {
    pub account: Account,
    pub hash: BlockHash,
    pub height: u64,
    pub num_blocks_confirmed: u64,
    pub block_callback_data: Vec<BlockHash>,
    pub source_block_callback_data: Vec<BlockHash>,
}
