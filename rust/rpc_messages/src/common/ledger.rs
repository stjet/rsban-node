use rsnano_core::{Account, Amount, BlockHash};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LedgerArgs {
    pub account: Account,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub representative: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receivable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_since: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sorting: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<Amount>,
}

impl LedgerArgs {
    pub fn new(account: Account, count: Option<u64>, representative: Option<bool>, weight: Option<bool>, receivable: Option<bool>, modified_since: Option<u64>, sorting: Option<bool>, threshold: Option<Amount>) -> Self {
        Self {
            account,
            count,
            representative,
            weight,
            receivable,
            modified_since,
            sorting,
            threshold
        }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LedgerDto {
    pub accounts: HashMap<Account, LedgerAccountInfo>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LedgerAccountInfo {
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

impl LedgerAccountInfo {
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