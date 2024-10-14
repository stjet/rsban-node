use rsnano_core::{Account, Amount, BlockHash, JsonBlock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationInfoDto {
    pub announcements: u32,
    pub voters: usize,
    pub last_winner: BlockHash,
    pub total_tally: Amount,
    pub final_tally: Amount,
    pub blocks: HashMap<BlockHash, ConfirmationBlockInfoDto>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationBlockInfoDto {
    pub tally: Amount,
    pub contents: Option<JsonBlock>,
    pub representatives: Option<HashMap<Account, Amount>>,
}

impl ConfirmationInfoDto {
    pub fn new(
        announcements: u32,
        voters: usize, // New parameter
        last_winner: BlockHash,
        total_tally: Amount,
        final_tally: Amount, // New parameter
        blocks: HashMap<BlockHash, ConfirmationBlockInfoDto>,
    ) -> Self {
        Self {
            announcements,
            voters,
            last_winner,
            total_tally,
            final_tally,
            blocks,
        }
    }
}
