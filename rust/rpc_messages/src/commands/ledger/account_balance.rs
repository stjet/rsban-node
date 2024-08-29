use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountBalanceArgs {
    pub account: Account,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_only_confirmed: Option<bool>,
}

#[cfg(test)]
mod tests {
    use crate::{AccountBalanceArgs, RpcCommand};
    use rsnano_core::Account;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_account_balance_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::account_balance(Account::zero(), None)).unwrap(),
            r#"{
  "action": "account_balance",
  "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp"
}"#
        )
    }

    #[test]
    fn serialize_account_balance_include_only_confirmed_some() {
        assert_eq!(
            to_string_pretty(&RpcCommand::account_balance(Account::zero(), Some(true))).unwrap(),
            r#"{
  "action": "account_balance",
  "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
  "include_only_confirmed": true
}"#
        )
    }

    #[test]
    fn deserialize_account_balance_include_only_confirmed_none() {
        let json_data = r#"{
                "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp"
            }"#;
        let deserialized: AccountBalanceArgs = from_str(json_data).unwrap();

        assert_eq!(
            deserialized,
            AccountBalanceArgs {
                account: Account::zero(),
                include_only_confirmed: None
            }
        );
    }

    #[test]
    fn deserialize_account_balance_include_only_confirmed_some() {
        let json_data = r#"{
                "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
                "include_only_confirmed": true
            }"#;
        let deserialized: AccountBalanceArgs = from_str(json_data).unwrap();

        assert_eq!(
            deserialized,
            AccountBalanceArgs {
                account: Account::zero(),
                include_only_confirmed: Some(true)
            }
        );
    }
}
