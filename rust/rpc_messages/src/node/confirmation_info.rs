use std::collections::HashMap;
use rsnano_core::{Account, Amount, BlockHash, JsonBlock, QualifiedRoot};
use serde::{Deserialize, Serialize};
use crate::RpcCommand;

impl RpcCommand {
    pub fn confirmation_info(root: QualifiedRoot, contents: Option<bool>, representatives: Option<bool>) -> Self {
        Self::ConfirmationInfo(ConfirmationInfoArgs::new(root, contents, representatives))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationInfoArgs {
    pub root: QualifiedRoot,
    pub contents: Option<bool>,
    pub representatives: Option<bool>,
}

impl ConfirmationInfoArgs {
    pub fn new(root: QualifiedRoot, contents: Option<bool>, representatives: Option<bool>) -> Self {
        Self {
            root,
            contents,
            representatives,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationInfoDto {
    pub announcements: u32,
    pub last_winner: BlockHash,
    pub total_tally: Amount,
    pub blocks: HashMap<BlockHash, BlockInfo>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlockInfo {
    pub tally: Amount,
    pub contents: Option<JsonBlock>,
    pub representatives: Option<HashMap<Account, Amount>>,
}

impl ConfirmationInfoDto {
    pub fn new(
        announcements: u32,
        last_winner: BlockHash,
        total_tally: Amount,
        blocks: HashMap<BlockHash, BlockInfo>,
    ) -> Self {
        Self {
            announcements,
            last_winner,
            total_tally,
            blocks,
        }
    }
}