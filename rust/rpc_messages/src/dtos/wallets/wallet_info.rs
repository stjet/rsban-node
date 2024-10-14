use rsnano_core::Amount;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletInfoDto {
    pub balance: Amount,
    pub pending: Amount,
    pub receivable: Amount,
    pub accounts_count: u64,
    pub adhoc_count: u64,
    pub deterministic_count: u64,
    pub deterministic_index: u32,
    pub accounts_block_count: u64,
    pub accounts_cemented_block_count: u64,
}

impl WalletInfoDto {
    pub fn new(
        balance: Amount,
        pending: Amount,
        receivable: Amount,
        accounts_count: u64,
        adhoc_count: u64,
        deterministic_count: u64,
        deterministic_index: u32,
        accounts_block_count: u64,
        accounts_cemented_block_count: u64,
    ) -> Self {
        Self {
            balance,
            pending,
            receivable,
            accounts_count,
            adhoc_count,
            deterministic_count,
            deterministic_index,
            accounts_block_count,
            accounts_cemented_block_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WalletInfoDto;
    use rsnano_core::Amount;

    #[test]
    fn serialize_wallet_info_dto() {
        let wallet_info = WalletInfoDto {
            balance: Amount::raw(1000),
            pending: Amount::raw(200),
            receivable: Amount::raw(300),
            accounts_count: 1,
            adhoc_count: 1,
            deterministic_count: 1,
            deterministic_index: 1,
            accounts_block_count: 1,
            accounts_cemented_block_count: 1,
        };

        let serialized = serde_json::to_string(&wallet_info).unwrap();

        assert_eq!(
            serialized,
            r#"{"balance":"1000","pending":"200","receivable":"300","accounts_count":1,"adhoc_count":1,"deterministic_count":1,"deterministic_index":1,"accounts_block_count":1,"accounts_cemented_block_count":1}"#
        );
    }

    #[test]
    fn deserialize_account_balance_dto() {
        let json_str = r#"{"balance":"1000","pending":"200","receivable":"300","accounts_count":1,"adhoc_count":1,"deterministic_count":1,"deterministic_index":1,"accounts_block_count":1,"accounts_cemented_block_count":1}"#;

        let deserialized: WalletInfoDto = serde_json::from_str(json_str).unwrap();

        let expected = WalletInfoDto {
            balance: Amount::raw(1000),
            pending: Amount::raw(200),
            receivable: Amount::raw(300),
            accounts_count: 1,
            adhoc_count: 1,
            deterministic_count: 1,
            deterministic_index: 1,
            accounts_block_count: 1,
            accounts_cemented_block_count: 1,
        };

        assert_eq!(deserialized, expected);
    }
}
