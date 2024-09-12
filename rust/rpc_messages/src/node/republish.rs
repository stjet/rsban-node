use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};
use crate::RpcCommand;

impl RpcCommand {
    pub fn republish(hash: BlockHash, sources: Option<bool>, destinations: Option<bool>) -> Self {
        Self::Republish(RepublishArgs::new(hash, sources, destinations))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]

pub struct RepublishArgs {
    pub hash: BlockHash,
    pub sources: Option<bool>,
    pub destinations: Option<bool>,
}

impl RepublishArgs {
    pub fn new(hash: BlockHash, sources: Option<bool>, destinations: Option<bool>) -> Self {
        Self {
            hash,
            sources,
            destinations,
        }
    }
}

