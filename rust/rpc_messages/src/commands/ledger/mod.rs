mod account_info;
mod account_list;

pub use account_info::*;
pub use account_list::*;
use rsnano_core::{Account, WalletId};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum LedgerRpcCommand {
    AccountInfo(AccountInfoArgs),
    AccountList(AccountListArgs),
}

impl LedgerRpcCommand {
    pub fn account_info(account: Account) -> Self {
        Self::AccountInfo(AccountInfoArgs { account })
    }

    pub fn account_list(wallet: WalletId) -> Self {
        Self::AccountList(AccountListArgs { wallet })
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
