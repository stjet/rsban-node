use rsnano_core::{Amount, BlockHash};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationHistoryArgs {
    pub hash: Option<BlockHash>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationHistoryResponse {
    pub confirmations: Vec<ConfirmationEntry>,
    pub confirmation_stats: ConfirmationStats,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationEntry {
    pub hash: BlockHash,
    pub duration: u64,
    pub time: u64,
    pub tally: Amount,
    #[serde(rename = "final")]
    pub final_tally: Amount,
    pub blocks: u32,
    pub voters: u32,
    pub request_count: u32,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationStats {
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average: Option<u64>,
}
