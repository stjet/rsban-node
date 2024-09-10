use crate::RpcCommand;
use rsnano_core::Account;
use rsnano_core::{Amount, BlockHash};
use serde::{Deserialize, Serialize};

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

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountInfoArgs {
    pub account: Account,
    pub representative: Option<bool>,
    pub weight: Option<bool>,
    pub pending: Option<bool>,
    pub receivable: Option<bool>,
    pub include_confirmed: Option<bool>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountInfoDto {
    pub frontier: BlockHash,
    pub open_block: BlockHash,
    pub representative_block: BlockHash,
    pub balance: Amount,
    pub modified_timestamp: u64,
    pub block_count: u64,
    pub account_version: u8,
    pub confirmed_height: Option<u64>,
    pub confirmation_height_frontier: Option<BlockHash>,
}

impl AccountInfoDto {
    pub fn new(
        frontier: BlockHash,
        open_block: BlockHash,
        representative_block: BlockHash,
        balance: Amount,
        modified_timestamp: u64,
        block_count: u64,
        account_version: u8,
    ) -> Self {
        Self {
            frontier,
            open_block,
            representative_block,
            balance,
            modified_timestamp,
            block_count,
            account_version,
            confirmed_height: None,
            confirmation_height_frontier: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::Account;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_account_info_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::account_info(
                Account::from(123),
                None,
                None,
                None,
                None,
                None
            ))
            .unwrap(),
            r#"{
  "action": "account_info",
  "account": "nano_111111111111111111111111111111111111111111111111115uwdgas549"
}"#
        )
    }

    #[test]
    fn derialize_account_info_command() {
        let account = Account::from(123);
        let cmd = RpcCommand::account_info(account, None, None, None, None, None);
        let serialized = to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
