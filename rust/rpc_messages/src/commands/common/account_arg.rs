use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountArg {
    pub account: Account,
}

impl AccountArg {
    pub fn new(account: Account) -> Self {
        Self { account }
    }
}

#[cfg(test)]
mod tests {
    use crate::AccountArg;
    use rsnano_core::Account;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_account_info_command() {
        assert_eq!(
            serde_json::to_string_pretty(&AccountArg::new(Account::from(123))).unwrap(),
            r#"{
  "account": "nano_111111111111111111111111111111111111111111111111115uwdgas549"
}"#
        )
    }

    #[test]
    fn derialize_account_info_command() {
        let account = Account::from(123);
        let account_arg = AccountArg::new(account);
        let serialized = to_string_pretty(&account_arg).unwrap();
        let deserialized: AccountArg = from_str(&serialized).unwrap();
        assert_eq!(account_arg, deserialized)
    }
}
