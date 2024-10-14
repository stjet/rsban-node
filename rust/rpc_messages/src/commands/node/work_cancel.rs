use crate::RpcCommand;
use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn work_cancel(hash: BlockHash) -> Self {
        Self::WorkCancel(WorkCancelArgs::new(hash))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WorkCancelArgs {
    pub hash: BlockHash,
}

impl WorkCancelArgs {
    pub fn new(hash: BlockHash) -> Self {
        Self { hash }
    }
}
