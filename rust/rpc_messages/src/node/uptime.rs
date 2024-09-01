use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
struct Uptime {
    seconds: u64,
}

impl Uptime {
    fn new(seconds: u64) -> Self {
        Self { seconds }
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
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
}
