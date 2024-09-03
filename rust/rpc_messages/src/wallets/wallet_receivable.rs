use crate::{RpcCommand, WalletWithCountArgs};
use rsnano_core::{Account, BlockHash, WalletId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

impl RpcCommand {
    pub fn wallet_receivable(wallet: WalletId, count: u64) -> Self {
        Self::WalletReceivable(WalletWithCountArgs::new(wallet, count))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletReceivableDto {
    pub blocks: HashMap<Account, BlockHash>,
}

impl WalletReceivableDto {
    pub fn new(blocks: HashMap<Account, BlockHash>) -> Self {
        Self { blocks }
    }
}

#[cfg(test)]
mod tests {
    use crate::{RpcCommand, WalletReceivableDto};
    use rsnano_core::{Account, BlockHash, WalletId};
    use serde_json::to_string_pretty;
    use std::collections::HashMap;

    #[test]
    fn serialize_wallet_receivable_dto() {
        let mut blocks = HashMap::new();
        blocks.insert(Account::zero(), BlockHash::zero());

        let works = WalletReceivableDto::new(blocks);

        let expected_json = r#"{"blocks":{"nano_1111111111111111111111111111111111111111111111111111hifc8npp":"0000000000000000000000000000000000000000000000000000000000000000"}}"#;
        let serialized = serde_json::to_string(&works).unwrap();

        assert_eq!(serialized, expected_json);
    }

    #[test]
    fn deserialize_wallet_receivable_dto() {
        let json_data = r#"{"blocks":{"nano_1111111111111111111111111111111111111111111111111111hifc8npp":"0000000000000000000000000000000000000000000000000000000000000000"}}"#;
        let works: WalletReceivableDto = serde_json::from_str(json_data).unwrap();

        let mut expected_blocks = HashMap::new();
        expected_blocks.insert(Account::zero(), BlockHash::zero());

        let expected_works = WalletReceivableDto {
            blocks: expected_blocks,
        };

        assert_eq!(works, expected_works);
    }

    #[test]
    fn serialize_wallet_receivable_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::wallet_receivable(WalletId::zero(), 1)).unwrap(),
            r#"{
  "action": "wallet_receivable",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000000",
  "count": 1
}"#
        )
    }

    #[test]
    fn deserialize_wallet_receivable_command() {
        let cmd = RpcCommand::wallet_receivable(WalletId::zero(), 1);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
