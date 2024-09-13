use crate::{RpcCommand, WalletWithCountArgs};
use rsnano_core::WalletId;

impl RpcCommand {
    pub fn wallet_receivable(wallet: WalletId, count: u64) -> Self {
        Self::WalletReceivable(WalletWithCountArgs::new(wallet, count))
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::WalletId;
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_wallet_receivable_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::wallet_receivable(WalletId::zero(), 1)).unwrap(),
            r#"{
  "action": "wallet_receivable",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000000",
  "count": 1
}"#
        )
    }

    #[test]
    fn deserialize_wallet_receivable_command() {
        let cmd = RpcCommand::wallet_receivable(WalletId::zero(), 1);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
