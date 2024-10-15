use crate::{CountRpcMessage, RpcCommand};

impl RpcCommand {
    pub fn unchecked(count: u64) -> Self {
        Self::Unchecked(CountRpcMessage::new(count))
    }
}

