use rsnano_core::{Account, BlockHash};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountInfoDto {
    pub frontier: BlockHash,
    pub open_block: BlockHash,
    pub representative_block: BlockHash,
    pub balance: u128,
    pub modified_timestamp: u64,
    pub block_count: u64,
    pub account_version: u8,
    pub confirmation_height: u64,
    pub confirmation_height_frontier: BlockHash,
    pub representative: Option<Account>,
    pub weight: Option<u128>,
    pub pending: Option<u128>,
    pub receivable: Option<u128>,
}
