use crate::RpcCommand;
use rsnano_core::Account;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn representatives_online(args: RepresentativesOnlineArgs) -> Self {
        Self::RepresentativesOnline(args)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RepresentativesOnlineArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accounts: Option<Vec<Account>>,
}

impl RepresentativesOnlineArgs {
    pub fn builder() -> RepresentativesOnlineArgsBuilder {
        RepresentativesOnlineArgsBuilder {
            args: RepresentativesOnlineArgs::default(),
        }
    }
}

pub struct RepresentativesOnlineArgsBuilder {
    args: RepresentativesOnlineArgs,
}

impl RepresentativesOnlineArgsBuilder {
    pub fn weight(mut self) -> Self {
        self.args.weight = Some(true);
        self
    }

    pub fn accounts(mut self, accounts: Vec<Account>) -> Self {
        self.args.accounts = Some(accounts);
        self
    }

    pub fn build(self) -> RepresentativesOnlineArgs {
        self.args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_representatives_online_command_options_none() {
        let command = RpcCommand::representatives_online(RepresentativesOnlineArgs::default());
        let serialized = serde_json::to_value(command).unwrap();
        let expected = json!({"action": "representatives_online"});
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_representatives_online_command_options_none() {
        let json = r#"{"action": "representatives_online"}"#;
        let deserialized: RpcCommand = serde_json::from_str(json).unwrap();
        let command = RpcCommand::representatives_online(RepresentativesOnlineArgs::default());
        assert_eq!(deserialized, command);
    }

    #[test]
    fn serialize_representatives_online_command_options_some() {
        let accounts = vec![Account::decode_account(
            "nano_1jg8zygjg3pp5w644emqcbmjqpnzmubfni3kfe1s8pooeuxsw49fdq1mco9j",
        )
        .unwrap()];
        let args = RepresentativesOnlineArgs::builder()
            .weight()
            .accounts(accounts.clone())
            .build();
        let command = RpcCommand::representatives_online(args);
        let serialized = serde_json::to_value(command).unwrap();
        let expected = json!({
            "action": "representatives_online",
            "weight": true,
            "accounts": ["nano_1jg8zygjg3pp5w644emqcbmjqpnzmubfni3kfe1s8pooeuxsw49fdq1mco9j"]
        });
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_representatives_online_command_options_some() {
        let json = r#"{
            "action": "representatives_online",
            "weight": true,
            "accounts": ["nano_1jg8zygjg3pp5w644emqcbmjqpnzmubfni3kfe1s8pooeuxsw49fdq1mco9j"]
        }"#;
        let deserialized: RpcCommand = serde_json::from_str(json).unwrap();
        if let RpcCommand::RepresentativesOnline(args) = deserialized {
            assert_eq!(args.weight, Some(true));
            assert_eq!(
                args.accounts,
                Some(vec![Account::decode_account(
                    "nano_1jg8zygjg3pp5w644emqcbmjqpnzmubfni3kfe1s8pooeuxsw49fdq1mco9j"
                )
                .unwrap()])
            );
        } else {
            panic!("Deserialized to wrong variant");
        }
    }
}
