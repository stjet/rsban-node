use rsnano_core::{Account, Amount, BlockHash, BlockSubType, Signature, WorkNonce};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountHistoryDto {
    pub account: Account,
    pub history: Vec<HistoryEntry>,
    pub previous: Option<BlockHash>,
    pub next: Option<BlockHash>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    #[serde(rename = "type")]
    pub block_type: BlockSubType,
    pub account: Account,
    pub amount: Amount,
    pub local_timestamp: u64,
    pub height: u64,
    pub hash: BlockHash,
    pub confirmed: bool,
    pub work: Option<WorkNonce>,
    pub signature: Option<Signature>,
}
