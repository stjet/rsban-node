use std::collections::VecDeque;

use rsnano_core::{Account, BlockHash};

pub struct ConfirmationHeightUnbounded {
    pub pending_writes: VecDeque<ConfHeightDetails>,
}

impl ConfirmationHeightUnbounded {
    pub fn new() -> Self {
        Self {
            pending_writes: VecDeque::new(),
        }
    }

    pub fn total_pending_write_block_count(&self) -> u64 {
        self.pending_writes
            .iter()
            .map(|x| x.num_blocks_confirmed)
            .sum()
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
