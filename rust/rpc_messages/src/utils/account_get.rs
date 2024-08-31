use crate::{KeyRpcMessage, RpcCommand};
use rsnano_core::PublicKey;

impl RpcCommand {
    pub fn account_get(key: PublicKey) -> Self {
        Self::AccountGet(KeyRpcMessage::new(key))
    }
}
