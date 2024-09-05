use std::collections::HashMap;
use crate::{ AccountsRpcMessage, RpcCommand};
use rsnano_core::Account;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn accounts_representatives(accounts: Vec<Account>) -> Self {
        Self::AccountsRepresentatives(AccountsRpcMessage::new(accounts))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsRepresentativesDto {
    pub representatives: HashMap<Account, Account>
}

impl AccountsRepresentativesDto {
    pub fn new(representatives: HashMap<Account, Account>) -> Self {
        Self { representatives }
    }
}


#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::Account;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_accounts_representatives_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::accounts_representatives(vec![Account::from(123)]))
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
    fn derialize_account_block_count_command() {
        let account = Account::from(123);
        let cmd = RpcCommand::accounts_representatives(vec![account]);
        let serialized = to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}

