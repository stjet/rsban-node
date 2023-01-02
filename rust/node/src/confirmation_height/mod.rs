use rsnano_core::{Account, BlockHash};
use std::collections::{HashMap, VecDeque};

pub struct ConfirmationHeightUnbounded {
    pub pending_writes: VecDeque<ConfHeightDetails>,
    pub confirmed_iterated_pairs: HashMap<Account, ConfirmedIteratedPair>,
}

impl ConfirmationHeightUnbounded {
    pub fn new() -> Self {
        Self {
            pending_writes: VecDeque::new(),
            confirmed_iterated_pairs: HashMap::new(),
        }
    }

    pub fn pending_empty(&self) -> bool {
        self.pending_writes.is_empty()
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

pub struct ConfirmedIteratedPair {
    pub confirmed_height: u64,
    pub iterated_height: u64,
}
