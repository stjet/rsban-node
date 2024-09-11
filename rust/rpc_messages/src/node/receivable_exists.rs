use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};
use crate::RpcCommand;

impl RpcCommand {
    pub fn receivable_exists(hash: BlockHash, include_active: Option<bool>, include_only_confirmed: Option<bool>) -> Self {
        Self::ReceivableExists(ReceivableExistsArgs::new(hash, include_active, include_only_confirmed))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]

pub struct ReceivableExistsArgs {
    pub hash: BlockHash,
    pub include_active: Option<bool>,
    pub include_only_confirmed: Option<bool>,
}

impl ReceivableExistsArgs {
    fn new(hash: BlockHash, include_active: Option<bool>, include_only_confirmed: Option<bool>) -> Self {
        Self { hash, include_active, include_only_confirmed }
    }
}