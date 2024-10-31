use crate::RpcCommand;
use rsnano_core::{Account, Amount};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

impl RpcCommand {
    pub fn unopened(args: UnopenedArgs) -> Self {
        Self::Unopened(args)
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub struct UnopenedArgs {
    pub account: Account,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<Amount>,
}

impl UnopenedArgs {
    pub fn new(account: Account) -> UnopenedArgs {
        UnopenedArgs {
            account,
            count: None,
            threshold: None,
        }
    }

    pub fn builder(account: Account) -> UnopenedArgsBuilder {
        UnopenedArgsBuilder {
            args: UnopenedArgs::new(account),
        }
    }
}

pub struct UnopenedArgsBuilder {
    args: UnopenedArgs,
}

impl UnopenedArgsBuilder {
    pub fn with_minimum_balance(mut self, threshold: Amount) -> Self {
        self.args.threshold = Some(threshold);
        self
    }

    pub fn build(self) -> UnopenedArgs {
        self.args
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UnopenedResponse {
    pub accounts: HashMap<Account, Amount>,
}

impl UnopenedResponse {
    pub fn new(accounts: HashMap<Account, Amount>) -> Self {
        Self { accounts }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::Account;
    use serde_json::{from_value, json, to_value};

    #[test]
    fn serialize_unopened_args_threshold_none() {
        let args = UnopenedArgs {
            account: Account::zero(),
            count: Some(1),
            ..Default::default()
        };
        let json = to_value(args).unwrap();

        assert_eq!(
            json,
            json!({
                "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
                "count": 1
            })
        );
    }

    #[test]
    fn serialize_unopened_args_threshold_some() {
        let args = UnopenedArgs {
            account: Account::zero(),
            count: Some(1),
            threshold: Some(Amount::zero()),
        };
        let json = to_value(args).unwrap();

        assert_eq!(
            json,
            json!({
                "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
                "count": 1,
                "threshold": "0"
            })
        );
    }

    #[test]
    fn serialize_unopened_command_threshold_none() {
        let command = RpcCommand::unopened(UnopenedArgs::new(Account::zero()));
        let json = to_value(command).unwrap();

        assert_eq!(
            json,
            json!({
                "action": "unopened",
                "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
            })
        );
    }

    #[test]
    fn deserialize_unopened_args_threshold_none() {
        let json = json!({
            "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
            "count": 1
        });

        let args: UnopenedArgs = from_value(json).unwrap();

        assert_eq!(
            args,
            UnopenedArgs {
                account: Account::zero(),
                count: Some(1),
                ..Default::default()
            }
        );
    }

    #[test]
    fn deserialize_unopened_command_threshold_some() {
        let json = json!({
            "action": "unopened",
            "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
            "count": 1,
            "threshold": "0"
        });

        let command: RpcCommand = from_value(json).unwrap();

        assert_eq!(
            command,
            RpcCommand::Unopened(UnopenedArgs {
                account: Account::zero(),
                count: Some(1),
                threshold: Some(Amount::zero())
            })
        );
    }
}
