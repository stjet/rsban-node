use crate::{common::HashRpcMessage, RpcCommand};
use rsban_core::BlockHash;

impl RpcCommand {
    pub fn work_cancel(hash: BlockHash) -> Self {
        Self::WorkCancel(HashRpcMessage::new(hash))
    }
}
