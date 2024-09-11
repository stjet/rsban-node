use rsnano_core::{Account, Amount, BlockHash, BlockSubType, WalletId};
use serde::{Deserialize, Serialize};
use crate::RpcCommand;

impl RpcCommand {
    pub fn wallet_history(wallet: WalletId, modified_since: Option<u64>) -> Self {
        Self::WalletHistory(WalletHistoryArgs::new(wallet, modified_since))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletHistoryArgs {
    pub wallet: WalletId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_since: Option<u64>,
}

impl WalletHistoryArgs {
    pub fn new(wallet: WalletId, modified_since: Option<u64>) -> Self {
        Self {
            wallet,
            modified_since,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletHistoryDto {
    pub history: Vec<HistoryEntry>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]

pub struct HistoryEntry {
    #[serde(rename = "type")]
    pub entry_type: BlockSubType,
    pub account: Account,
    pub amount: Amount,
    pub block_account: Account,
    pub hash: BlockHash,
    pub local_timestamp: u64,
}

