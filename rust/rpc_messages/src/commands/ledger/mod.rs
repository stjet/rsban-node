mod account_balance;
mod account_info;

pub use account_balance::*;
pub use account_info::*;
use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum LedgerRpcCommand {
    AccountBalance(AccountBalanceArgs),
    AccountInfo(AccountInfoArgs),
}

impl LedgerRpcCommand {
    pub fn account_balance(account: Account, include_only_confirmed: Option<bool>) -> Self {
        Self::AccountBalance(AccountBalanceArgs {
            account,
            include_only_confirmed,
        })
    }

    pub fn account_info(account: Account) -> Self {
        Self::AccountInfo(AccountInfoArgs { account })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::Account;

    #[test]
    fn deserialize() {
        let account = Account::from(123);
        let cmd = LedgerRpcCommand::account_info(account);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: LedgerRpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
