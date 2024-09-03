use crate::{RpcCommand, WalletRpcMessage};
use rsnano_core::WalletId;

impl RpcCommand {
    pub fn wallet_frontiers(wallet: WalletId) -> Self {
        Self::WalletFrontiers(WalletRpcMessage::new(wallet))
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::Account;
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_wallet_frontiers_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::wallet_frontiers(vec![Account::zero()])).unwrap(),
            r#"{
  "action": "wallet_frontiers",
  "wallet": ""
}"#
        )
    }

    #[test]
    fn deserialize_wallet_frontiers_command() {
        let json_str = r#"{
    "action": "wallet_frontiers",
    "wallet": ""
    }"#;
        let deserialized: RpcCommand = serde_json::from_str(json_str).unwrap();
        let expected_command = RpcCommand::wallet_frontiers(vec![Account::zero()]);
        assert_eq!(deserialized, expected_command);
    }
}
