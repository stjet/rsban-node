use rsnano_core::{Account, Amount, BlockHash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletLedgerDto {
    pub accounts: HashMap<Account, AccountInfo>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountInfo {
    pub frontier: BlockHash,
    pub open_block: BlockHash,
    pub representative_block: BlockHash,
    pub balance: Amount,
    pub modified_timestamp: u64,
    pub block_count: u64,
    pub representative: Option<Account>,
    pub weight: Option<Amount>,
    pub pending: Option<Amount>,
    pub receivable: Option<Amount>,
}

impl AccountInfo {
    pub fn new(
        frontier: BlockHash,
        open_block: BlockHash,
        representative_block: BlockHash,
        balance: Amount,
        modified_timestamp: u64,
        block_count: u64,
        representative: Option<Account>,
        weight: Option<Amount>,
        pending: Option<Amount>,
        receivable: Option<Amount>,
    ) -> Self {
        Self {
            frontier,
            open_block,
            representative_block,
            balance,
            modified_timestamp,
            block_count,
            representative,
            weight,
            pending,
            receivable,
        }
    }
}
