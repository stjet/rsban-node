use crate::RpcCommand;
use rsnano_core::WalletId;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn account_create(wallet: WalletId, index: Option<u32>) -> Self {
        Self::AccountCreate(AccountCreateArgs { wallet, index })
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountCreateArgs {
    pub wallet: WalletId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::WalletId;
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_account_create_command_index_none() {
        assert_eq!(
            to_string_pretty(&RpcCommand::account_create(1.into(), None)).unwrap(),
            r#"{
  "action": "account_create",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000001"
}"#
        )
    }

    #[test]
    fn serialize_account_create_command_index_some() {
        assert_eq!(
            to_string_pretty(&RpcCommand::account_create(1.into(), Some(1))).unwrap(),
            r#"{
  "action": "account_create",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000001",
  "index": 1
}"#
        )
    }

    #[test]
    fn deserialize_account_create_command_index_none() {
        let wallet = WalletId::random();
        let cmd = RpcCommand::account_create(wallet, None);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }

    #[test]
    fn deserialize_account_create_command_index_some() {
        let wallet = WalletId::random();
        let cmd = RpcCommand::account_create(wallet, Some(1));
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
