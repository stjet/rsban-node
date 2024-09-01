use crate::{AccountRpcMessage, RpcCommand};
use rsnano_core::Account;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn account_block_count(account: Account) -> Self {
        Self::AccountBlockCount(AccountRpcMessage::new("account".to_string(), account))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountBlockCountDto {
    pub block_count: u64,
}

impl AccountBlockCountDto {
    pub fn new(block_count: u64) -> Self {
        Self { block_count }
    }
}

#[cfg(test)]
mod tests {
    use crate::{AccountBlockCountDto, RpcCommand};
    use rsnano_core::Account;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_account_block_count_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::account_block_count(Account::from(123)))
                .unwrap(),
            r#"{
  "action": "account_block_count",
  "account": "nano_111111111111111111111111111111111111111111111111115uwdgas549"
}"#
        )
    }

    #[test]
    fn derialize_account_block_count_command() {
        let account = Account::from(123);
        let cmd = RpcCommand::account_block_count(account);
        let serialized = to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }

    #[test]
    fn deserialize_account_block_count_dto() {
        let block_count_dto = AccountBlockCountDto::new(1);
        let serialized = to_string_pretty(&block_count_dto).unwrap();
        let deserialized: AccountBlockCountDto = from_str(&serialized).unwrap();
        assert_eq!(block_count_dto, deserialized);
    }

    #[test]
    fn serialize_account_block_count_dto() {
        assert_eq!(
            serde_json::to_string_pretty(&AccountBlockCountDto::new(1)).unwrap(),
            r#"{
  "block_count": 1
}"#
        )
    }
}
