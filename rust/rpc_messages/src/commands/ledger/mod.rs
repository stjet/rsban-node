mod account_info;

pub use account_info::*;
use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum LedgerRpcCommand {
    AccountInfo(AccountInfoArgs),
}

impl LedgerRpcCommand {
    pub fn account_info(
        account: Account,
        representative: Option<bool>,
        weight: Option<bool>,
        pending: Option<bool>,
        receivable: Option<bool>,
        include_confirmed: Option<bool>,
    ) -> Self {
        Self::AccountInfo(AccountInfoArgs {
            account,
            representative,
            weight,
            pending,
            receivable,
            include_confirmed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::Account;

    #[test]
    fn deserialize() {
        let account = Account::from(123);
        let cmd = LedgerRpcCommand::account_info(account, None, None, None, None, None);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: LedgerRpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
