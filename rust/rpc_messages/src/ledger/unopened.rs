use crate::{AccountWithCountArgs, RpcCommand};
use rsnano_core::{Account, Amount};
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn unopened(args: UnopenedArgs) -> Self {
        Self::Unopened(args)
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UnopenedArgs {
    #[serde(flatten)]
    pub account_with_count: AccountWithCountArgs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<Amount>,
}

impl UnopenedArgs {
    pub fn new(account: Account, count: u64) -> UnopenedArgs {
        UnopenedArgs { account_with_count: AccountWithCountArgs::new(account, count), threshold: None }
    }

    pub fn builder(account: Account, count: u64) -> UnopenedArgsBuilder {
        UnopenedArgsBuilder::new(account, count)
    }
}

pub struct UnopenedArgsBuilder {
    args: UnopenedArgs,
}

impl UnopenedArgsBuilder {
    fn new(account: Account, count: u64) -> Self {
        Self {
            args: UnopenedArgs {
                account_with_count: AccountWithCountArgs::new(account, count),
                threshold: None,
            },
        }
    }

    pub fn with_minimum_balance(mut self, threshold: Amount) -> Self {
        self.args.threshold = Some(threshold);
        self
    }

    pub fn build(self) -> UnopenedArgs {
        self.args
    }
}

impl From<AccountWithCountArgs> for UnopenedArgs {
    fn from(account_with_count: AccountWithCountArgs) -> Self {
        Self {
            account_with_count,
            threshold: None,
        }
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
            account_with_count: AccountWithCountArgs {
                account: Account::zero(),
                count: 1,
            },
            threshold: None,
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
            account_with_count: AccountWithCountArgs {
                account: Account::zero(),
                count: 1,
            },
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
        let args = UnopenedArgs {
            account_with_count: AccountWithCountArgs {
                account: Account::zero(),
                count: 1,
            },
            threshold: None,
        };
        let command = RpcCommand::unopened(args);
        let json = to_value(command).unwrap();

        assert_eq!(
            json,
            json!({
                "action": "unopened",
                "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
                "count": 1
            })
        );
    }

    #[test]
    fn serialize_unopened_command_threshold_some() {
        let args = UnopenedArgs {
            account_with_count: AccountWithCountArgs {
                account: Account::zero(),
                count: 1,
            },
            threshold: Some(Amount::zero()),
        };
        let command = RpcCommand::unopened(args);
        let json = to_value(command).unwrap();

        assert_eq!(
            json,
            json!({
                "action": "unopened",
                "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
                "count": 1,
                "threshold": "0"
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
                account_with_count: AccountWithCountArgs {
                    account: Account::zero(),
                    count: 1,
                },
                threshold: None,
            }
        );
    }

    #[test]
    fn deserialize_unopened_args_threshold_some() {
        let json = json!({
            "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
            "count": 1,
            "threshold": "0"
        });

        let args: UnopenedArgs = from_value(json).unwrap();

        assert_eq!(
            args,
            UnopenedArgs {
                account_with_count: AccountWithCountArgs {
                    account: Account::zero(),
                    count: 1,
                },
                threshold: Some(Amount::zero()),
            }
        );
    }

    #[test]
    fn deserialize_unopened_command_threshold_none() {
        let json = json!({
            "action": "unopened",
            "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
            "count": 1
        });

        let command: RpcCommand = from_value(json).unwrap();

        assert_eq!(
            command,
            RpcCommand::Unopened(UnopenedArgs {
                account_with_count: AccountWithCountArgs {
                    account: Account::zero(),
                    count: 1,
                },
                threshold: None,
            })
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
                account_with_count: AccountWithCountArgs {
                    account: Account::zero(),
                    count: 1,
                },
                threshold: Some(Amount::zero()),
            })
        );
    }

    #[test]
    fn unopened_args_builder() {
        let args = UnopenedArgs::builder(Account::zero(), 5)
            .with_minimum_balance(Amount::from(100))
            .build();

        assert_eq!(args.account_with_count.account, Account::zero());
        assert_eq!(args.account_with_count.count, 5);
        assert_eq!(args.threshold, Some(Amount::from(100)));
    }

    #[test]
    fn unopened_command_with_builder() {
        let command = RpcCommand::unopened(UnopenedArgs::builder(Account::zero(), 3).build());

        if let RpcCommand::Unopened(args) = command {
            assert_eq!(args.account_with_count.account, Account::zero());
            assert_eq!(args.account_with_count.count, 3);
            assert_eq!(args.threshold, None);
        } else {
            panic!("Expected RpcCommand::Unopened variant");
        }
    }
}
