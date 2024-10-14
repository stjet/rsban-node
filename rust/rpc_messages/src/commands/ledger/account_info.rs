use crate::RpcCommand;
use rsnano_core::Account;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn account_info(account_info_args: AccountInfoArgs) -> Self {
        Self::AccountInfo(account_info_args)
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountInfoArgs {
    pub account: Account,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub representative: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receivable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_confirmed: Option<bool>,
}

impl AccountInfoArgs {
    pub fn new(account: Account) -> AccountInfoArgs {
        AccountInfoArgs {
            account,
            representative: None,
            weight: None,
            pending: None,
            receivable: None,
            include_confirmed: None,
        }
    }

    pub fn builder(account: Account) -> AccountInfoArgsBuilder {
        AccountInfoArgsBuilder::new(account)
    }
}

impl From<Account> for AccountInfoArgs {
    fn from(account: Account) -> Self {
        Self {
            account,
            representative: None,
            weight: None,
            pending: None,
            receivable: None,
            include_confirmed: None,
        }
    }
}

pub struct AccountInfoArgsBuilder {
    args: AccountInfoArgs,
}

impl AccountInfoArgsBuilder {
    fn new(account: Account) -> Self {
        Self {
            args: AccountInfoArgs {
                account,
                representative: None,
                weight: None,
                pending: None,
                receivable: None,
                include_confirmed: None,
            },
        }
    }

    pub fn include_representative(mut self) -> Self {
        self.args.representative = Some(true);
        self
    }

    pub fn include_weight(mut self) -> Self {
        self.args.weight = Some(true);
        self
    }

    pub fn include_pending(mut self) -> Self {
        self.args.pending = Some(true);
        self
    }

    pub fn include_receivable(mut self) -> Self {
        self.args.receivable = Some(true);
        self
    }

    pub fn include_confirmed(mut self) -> Self {
        self.args.include_confirmed = Some(true);
        self
    }

    pub fn build(self) -> AccountInfoArgs {
        self.args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn test_account_info_args_builder() {
        let account = Account::from(123);
        let args = AccountInfoArgs::builder(account)
            .include_weight()
            .include_pending()
            .include_confirmed()
            .build();

        assert_eq!(args.account, account);
        assert_eq!(args.representative, None);
        assert_eq!(args.weight, Some(true));
        assert_eq!(args.pending, Some(true));
        assert_eq!(args.receivable, None);
        assert_eq!(args.include_confirmed, Some(true));
    }

    #[test]
    fn serialize_account_info_command_with_optionals() {
        let account = Account::from(123);
        let args = AccountInfoArgs::builder(account)
            .include_representative()
            .include_weight()
            .include_pending()
            .include_receivable()
            .include_confirmed()
            .build();

        let serialized = to_string_pretty(&RpcCommand::account_info(args)).unwrap();

        assert!(serialized.contains(r#""action": "account_info""#));
        assert!(serialized.contains(
            r#""account": "nano_111111111111111111111111111111111111111111111111115uwdgas549""#
        ));
        assert!(serialized.contains(r#""representative": true"#));
        assert!(serialized.contains(r#""weight": true"#));
        assert!(serialized.contains(r#""pending": true"#));
        assert!(serialized.contains(r#""receivable": true"#));
        assert!(serialized.contains(r#""include_confirmed": true"#));
    }

    #[test]
    fn deserialize_account_info_command_with_optionals() {
        let json = r#"{
            "action": "account_info",
            "account": "nano_111111111111111111111111111111111111111111111111115uwdgas549",
            "representative": true,
            "weight": true,
            "pending": true,
            "receivable": true,
            "include_confirmed": true
        }"#;

        let deserialized: RpcCommand = from_str(json).unwrap();

        if let RpcCommand::AccountInfo(args) = deserialized {
            assert_eq!(args.account, Account::from(123));
            assert_eq!(args.representative, Some(true));
            assert_eq!(args.weight, Some(true));
            assert_eq!(args.pending, Some(true));
            assert_eq!(args.receivable, Some(true));
            assert_eq!(args.include_confirmed, Some(true));
        } else {
            panic!("Deserialized to wrong RpcCommand variant");
        }
    }

    #[test]
    fn serialize_account_info_command_without_optionals() {
        let account = Account::from(123);
        let args = AccountInfoArgs::builder(account).build();

        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::account_info(args)).unwrap(),
            r#"{
  "action": "account_info",
  "account": "nano_111111111111111111111111111111111111111111111111115uwdgas549"
}"#
        )
    }

    #[test]
    fn derialize_account_info_command_without_optionals() {
        let account = Account::from(123);
        let args = AccountInfoArgs::builder(account).build();
        let cmd = RpcCommand::account_info(args);
        let serialized = to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = from_str(&serialized).unwrap();

        assert_eq!(cmd, deserialized)
    }

    #[test]
    fn serialize_account_info_args() {
        let args = AccountInfoArgs::builder(Account::zero())
            .include_representative()
            .include_weight()
            .include_receivable()
            .build();

        let serialized = to_string_pretty(&args).unwrap();
        let deserialized: AccountInfoArgs = from_str(&serialized).unwrap();

        assert!(serialized.contains("account"));
        assert!(serialized.contains("representative"));
        assert!(serialized.contains("weight"));
        assert!(serialized.contains("receivable"));
        assert!(!serialized.contains("pending"));
        assert!(!serialized.contains("include_confirmed"));
        assert_eq!(args, deserialized);
    }

    #[test]
    fn serialize_account_info_command_with_some_args() {
        let args = AccountInfoArgs::builder(Account::zero())
            .include_representative()
            .include_weight()
            .include_receivable()
            .build();

        let command = RpcCommand::account_info(args);
        let serialized = to_string_pretty(&command).unwrap();

        assert!(serialized.contains("account"));
        assert!(serialized.contains("representative"));
        assert!(serialized.contains("weight"));
        assert!(serialized.contains("receivable"));
        assert!(!serialized.contains("pending"));
        assert!(!serialized.contains("include_confirmed"));
    }
}
