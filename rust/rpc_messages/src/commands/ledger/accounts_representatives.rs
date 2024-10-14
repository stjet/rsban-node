use crate::RpcCommand;
use rsnano_core::Account;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn accounts_representatives(accounts: Vec<Account>) -> Self {
        Self::AccountsRepresentatives(AccountsRepresentativesArgs::new(accounts))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsRepresentativesArgs {
    pub accounts: Vec<Account>,
}

impl AccountsRepresentativesArgs {
    pub fn new(accounts: Vec<Account>) -> Self {
        Self { accounts }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::Account;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_accounts_representatives_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::accounts_representatives(vec![Account::from(
                123
            )]))
            .unwrap(),
            r#"{
  "action": "accounts_representatives",
  "accounts": [
    "nano_111111111111111111111111111111111111111111111111115uwdgas549"
  ]
}"#
        )
    }

    #[test]
    fn deserialize_accounts_representatives_command() {
        let account = Account::from(123);
        let cmd = RpcCommand::accounts_representatives(vec![account]);
        let serialized = to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
