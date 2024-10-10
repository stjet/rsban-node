use crate::RpcCommand;
use rsnano_core::Account;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn account_balance(account_balance_args: AccountBalanceArgs) -> Self {
        Self::AccountBalance(account_balance_args)
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountBalanceArgs {
    pub account: Account,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_only_confirmed: Option<bool>,
}

impl AccountBalanceArgs {
    pub fn builder(account: Account) -> AccountBalanceArgsBuilder {
        AccountBalanceArgsBuilder::new(account)
    }
}

impl From<Account> for AccountBalanceArgs {
    fn from(account: Account) -> Self {
        Self {
            account,
            include_only_confirmed: None,
        }
    }
}

pub struct AccountBalanceArgsBuilder {
    args: AccountBalanceArgs,
}

impl AccountBalanceArgsBuilder {
    fn new(account: Account) -> Self {
        Self {
            args: AccountBalanceArgs {
                account,
                include_only_confirmed: None,
            },
        }
    }

    pub fn include_only_confirmed(mut self) -> Self {
        self.args.include_only_confirmed = Some(true);
        self
    }

    pub fn finish(self) -> AccountBalanceArgs {
        self.args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::to_string_pretty;

    #[test]
    fn test_account_balance_args_builder() {
        let account = Account::from(123);
        let args = AccountBalanceArgs::builder(account)
            .include_only_confirmed()
            .finish();

        assert_eq!(args.account, account);
        assert_eq!(args.include_only_confirmed, Some(true));
    }

    #[test]
    fn serialize_account_balance_command_with_builder() {
        let account = Account::from(123);
        let args = AccountBalanceArgs::builder(account)
            .include_only_confirmed()
            .finish();
        
        let serialized = to_string_pretty(&RpcCommand::account_balance(args)).unwrap();
        
        assert!(serialized.contains(r#""action": "account_balance""#));
        assert!(serialized.contains(r#""account": "nano_111111111111111111111111111111111111111111111111115uwdgas549""#));
        assert!(serialized.contains(r#""include_only_confirmed": true"#));
    }

    #[test]
    fn deserialize_account_balance_command_with_builder() {
        let json = r#"{
            "action": "account_balance",
            "account": "nano_111111111111111111111111111111111111111111111111115uwdgas549",
            "include_only_confirmed": true
        }"#;

        let deserialized: RpcCommand = serde_json::from_str(json).unwrap();

        if let RpcCommand::AccountBalance(args) = deserialized {
            assert_eq!(args.account, Account::from(123));
            assert_eq!(args.include_only_confirmed, Some(true));
        } else {
            panic!("Deserialized to wrong RpcCommand variant");
        }
    }

    #[test]
    fn serialize_account_balance_command_include_only_confirmed_none() {
        let account_balance_args = AccountBalanceArgsBuilder::new(Account::zero()).finish();
        assert_eq!(
            to_string_pretty(&RpcCommand::account_balance(account_balance_args)).unwrap(),
            r#"{
  "action": "account_balance",
  "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp"
}"#
        )
    }

    #[test]
    fn serialize_account_balance_command_include_only_confirmed_some() {
        let account_balance_args = AccountBalanceArgsBuilder::new(Account::zero()).include_only_confirmed().finish();
        assert_eq!(
            to_string_pretty(&RpcCommand::account_balance(account_balance_args)).unwrap(),
            r#"{
  "action": "account_balance",
  "account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
  "include_only_confirmed": true
}"#
        )
    }

    #[test]
    fn deserialize_account_balance_command_include_only_confirmed_none() {
        let account_balance_args = AccountBalanceArgsBuilder::new(Account::zero()).finish();
        let cmd = RpcCommand::account_balance(account_balance_args);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }

    #[test]
    fn deserialize_account_balance_command_include_only_confirmed_some() {
        let account_balance_args = AccountBalanceArgsBuilder::new(Account::zero()).finish();
        let cmd = RpcCommand::account_balance(account_balance_args);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }

    #[test]
    fn test_from_account_for_account_balance_args() {
        let account = Account::from(123);
        let args: AccountBalanceArgs = account.into();

        assert_eq!(args.account, account);
        assert_eq!(args.include_only_confirmed, None);
    }
}
