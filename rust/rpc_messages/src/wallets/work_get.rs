use crate::{RpcCommand, WalletWithAccountArgs};
use rsnano_core::{Account, WalletId};

impl RpcCommand {
    pub fn work_get(wallet: WalletId, account: Account) -> Self {
        Self::WorkGet(WalletWithAccountArgs::new(wallet, account))
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::{Account, WalletId};
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_work_get_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::work_get(WalletId::zero(), Account::zero())).unwrap(),
            r#"{
  "action": "work_get",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000000",
  "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp"
}"#
        )
    }

    #[test]
    fn deserialize_work_get_command() {
        let cmd = RpcCommand::work_get(WalletId::zero(), Account::zero());
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
