use crate::{HashRpcMessage, RpcCommand};
use rsnano_core::BlockHash;

impl RpcCommand {
    pub fn unchecked_get(hash: BlockHash) -> Self {
        Self::UncheckedGet(HashRpcMessage::new(hash))
    }
}