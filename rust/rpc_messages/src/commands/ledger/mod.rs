mod account_info;

use super::RpcCommand;
pub use account_info::*;
use rsnano_core::Account;

impl RpcCommand {
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
        let cmd = RpcCommand::account_info(account, None, None, None, None, None);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
