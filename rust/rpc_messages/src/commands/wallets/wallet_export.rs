use crate::{RpcCommand, WalletRpcMessage};
use rsnano_core::WalletId;

impl RpcCommand {
    pub fn wallet_export(wallet: WalletId) -> Self {
        Self::WalletExport(WalletRpcMessage::new(wallet))
    }
}

#[cfg(test)]
mod tests {
    use super::RpcCommand;
    use rsnano_core::WalletId;
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_wallet_export_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::wallet_export(WalletId::zero())).unwrap(),
            r#"{
  "action": "wallet_export",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000000"
}"#
        )
    }

    #[test]
    fn deserialize_wallet_export_command() {
        let cmd = RpcCommand::wallet_export(WalletId::zero());
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
