use crate::{RpcCommand, WalletRpcMessage};
use rsnano_core::WalletId;

impl RpcCommand {
    pub fn wallet_work_get(wallet: WalletId) -> Self {
        Self::WalletWorkGet(WalletRpcMessage::new(wallet))
    }
}

#[cfg(test)]
mod tests {
    use rsnano_core::WalletId;
    use serde_json::to_string_pretty;

    use crate::RpcCommand;

    #[test]
    fn serialize_wallet_work_get_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::wallet_work_get(WalletId::zero(),)).unwrap(),
            r#"{
  "action": "wallet_work_get",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000000"
}"#
        )
    }

    #[test]
    fn deserialize_wallet_work_get_command() {
        let cmd = RpcCommand::wallet_work_get(WalletId::zero());
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
