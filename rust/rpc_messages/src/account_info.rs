use rsnano_core::{Account, Amount, BlockHash};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountInfoRequest {
    pub account: Account,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountInfoResponse {
    pub frontier: BlockHash,
    pub block_count: u64,
    pub balance: Amount,
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::Account;

    #[test]
    fn serialize_account_info_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::account_info(Account::from(123))).unwrap(),
            r#"{
  "action": "account_info",
  "account": "nano_111111111111111111111111111111111111111111111111115uwdgas549"
}"#
        )
    }
}
