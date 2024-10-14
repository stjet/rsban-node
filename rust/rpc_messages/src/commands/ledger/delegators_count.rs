use crate::RpcCommand;
use rsnano_core::Account;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn delegators_count(account: Account) -> Self {
        Self::DelegatorsCount(DelegatorsCountArgs::new(account))
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DelegatorsCountArgs {
    pub account: Account
}

impl DelegatorsCountArgs {
    pub fn new(account: Account) -> Self {
        Self { account }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_delegators_count_command() {
        let command = RpcCommand::delegators_count(Account::zero());
        let serialized = serde_json::to_value(command).unwrap();
        let expected = json!({"action": "delegators_count", "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp"});
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_delegators_count_command() {
        let json = r#"{"action": "delegators_count","account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp"}"#;
        let deserialized: RpcCommand = serde_json::from_str(json).unwrap();
        let expected = RpcCommand::delegators_count(Account::zero());
        assert_eq!(deserialized, expected);
    }
}
