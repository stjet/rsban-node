use crate::RpcCommand;
use rsnano_core::WalletId;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn accounts_create(wallet: WalletId, count: u64) -> Self {
        Self::AccountsCreate(AccountsCreateArgs { wallet, count })
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsCreateArgs {
    pub wallet: WalletId,
    pub count: u64,
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::WalletId;
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_accounts_create_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::accounts_create(1.into(), 2)).unwrap(),
            r#"{
  "action": "accounts_create",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000001",
  "count": 2
}"#
        )
    }

    #[test]
    fn deserialize_accounts_create_command_index_none() {
        let cmd = RpcCommand::accounts_create(1.into(), 2);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
