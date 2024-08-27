use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountInfoArgs {
    pub account: Account,
    pub representative: Option<bool>,
    pub weight: Option<bool>,
    pub pending: Option<bool>,
    pub receivable: Option<bool>,
    pub include_confirmed: Option<bool>,
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::Account;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_account_info_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::account_info(
                Account::from(123),
                None,
                None,
                None,
                None,
                None
            ))
            .unwrap(),
            r#"{
  "action": "account_info",
  "account": "nano_111111111111111111111111111111111111111111111111115uwdgas549"
}"#
        )
    }

    #[test]
    fn derialize_account_info_command() {
        let account = Account::from(123);
        let cmd = RpcCommand::account_info(account, None, None, None, None, None);
        let serialized = to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
