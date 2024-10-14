use crate::RpcCommand;
use rsnano_core::Account;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn accounts_frontiers(accounts: Vec<Account>) -> Self {
        Self::AccountsFrontiers(AccountsFrontiersArgs::new(accounts))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsFrontiersArgs {
    pub accounts: Vec<Account>
}

impl AccountsFrontiersArgs {
    pub fn new(accounts: Vec<Account>) -> Self {
        Self { accounts }
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::Account;
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_accounts_frontiers_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::accounts_frontiers(vec![Account::zero()])).unwrap(),
            r#"{
  "action": "accounts_frontiers",
  "accounts": [
    "nano_1111111111111111111111111111111111111111111111111111hifc8npp"
  ]
}"#
        )
    }

    #[test]
    fn deserialize_accounts_frontiers_command() {
        let json_str = r#"{
    "action": "accounts_frontiers",
    "accounts": ["nano_1111111111111111111111111111111111111111111111111111hifc8npp"]
    }"#;
        let deserialized: RpcCommand = serde_json::from_str(json_str).unwrap();
        let expected_command = RpcCommand::accounts_frontiers(vec![Account::zero()]);
        assert_eq!(deserialized, expected_command);
    }
}
