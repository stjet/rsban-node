use crate::RpcCommand;
use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn unchecked_get(hash: BlockHash) -> Self {
        Self::UncheckedGet(UncheckedGetArgs::new(hash))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UncheckedGetArgs {
    pub hash: BlockHash,
}

impl UncheckedGetArgs {
    pub fn new(hash: BlockHash) -> Self {
        Self { hash }
    }
}
