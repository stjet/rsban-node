use crate::{RpcCommand, WalletRpcMessage};
use rsnano_core::WalletId;

impl RpcCommand {
    pub fn wallet_info(wallet: WalletId) -> Self {
        Self::WalletInfo(WalletRpcMessage::new(wallet))
    }
}

#[cfg(test)]
mod tests {
    use super::RpcCommand;
    use rsnano_core::WalletId;
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_wallet_info() {
        assert_eq!(
            to_string_pretty(&RpcCommand::wallet_info(WalletId::zero())).unwrap(),
            r#"{
  "action": "wallet_info",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000000"
}"#
        )
    }

    #[test]
    fn deserialize_wallet_info() {
        let cmd = RpcCommand::wallet_info(WalletId::zero());
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
