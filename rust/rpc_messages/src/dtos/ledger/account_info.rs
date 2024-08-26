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

impl AccountInfoDto {
    pub fn new(
        frontier: BlockHash,
        open_block: BlockHash,
        representative_block: BlockHash,
        balance: u128,
        modified_timestamp: u64,
        block_count: u64,
        account_version: u8,
        confirmation_height: u64,
        confirmation_height_frontier: BlockHash,
        representative: Option<Account>,
        weight: Option<u128>,
        pending: Option<u128>,
        receivable: Option<u128>,
    ) -> Self {
        AccountInfoDto {
            frontier,
            open_block,
            representative_block,
            balance,
            modified_timestamp,
            block_count,
            account_version,
            confirmation_height,
            confirmation_height_frontier,
            representative,
            weight,
            pending,
            receivable,
        }
    }
}
