use rsnano_core::{Account, Amount, BlockHash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LedgerDto {
    pub accounts: HashMap<Account, LedgerAccountInfo>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LedgerAccountInfo {
    pub frontier: BlockHash,
    pub open_block: BlockHash,
    pub representative_block: BlockHash,
    pub balance: Amount,
    pub modified_timestamp: u64,
    pub block_count: u64,
    pub representative: Option<Account>,
    pub weight: Option<Amount>,
    pub pending: Option<Amount>,
    pub receivable: Option<Amount>,
}

impl LedgerAccountInfo {
    pub fn new(
        frontier: BlockHash,
        open_block: BlockHash,
        representative_block: BlockHash,
        balance: Amount,
        modified_timestamp: u64,
        block_count: u64,
        representative: Option<Account>,
        weight: Option<Amount>,
        pending: Option<Amount>,
        receivable: Option<Amount>,
    ) -> Self {
        Self {
            frontier,
            open_block,
            representative_block,
            balance,
            modified_timestamp,
            block_count,
            representative,
            weight,
            pending,
            receivable,
        }
    }
}

#[cfg(test)]
mod tests {
    use rsnano_core::{Account, Amount, BlockHash};
    use serde_json::json;
    use std::collections::HashMap;

    use crate::dtos::{LedgerAccountInfo, LedgerDto};

    #[test]
    fn test_ledger_dto_serialization() {
        let mut accounts = HashMap::new();
        accounts.insert(
            Account::decode_account(
                "nano_1ipx847tk8o46pwxt5qjdbncjqcbwcc1rrmqnkztrfjy5k7z4imsrata9est",
            )
            .unwrap(),
            LedgerAccountInfo::new(
                BlockHash::decode_hex(
                    "000D1BAEC8EC208142C99059B393051BAC8380F9B5A2E6B2489A277D81789F3F",
                )
                .unwrap(),
                BlockHash::decode_hex(
                    "991CF190094C00F0B68E2E5F75F6BEE95A2E0BD93CEAA4A6734DB9F19C34F1ED",
                )
                .unwrap(),
                BlockHash::decode_hex(
                    "991CF190094C00F0B68E2E5F75F6BEE95A2E0BD93CEAA4A6734DB9F19C34F1ED",
                )
                .unwrap(),
                Amount::raw(10000000000000000000000000000000u128),
                1553174994,
                50,
                Some(
                    Account::decode_account(
                        "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
                    )
                    .unwrap(),
                ),
                Some(Amount::raw(10000000000000000000000000000000u128)),
                Some(Amount::raw(10000000000000000000000000000u128)),
                Some(Amount::raw(10000000000000000000000000000u128)),
            ),
        );

        let ledger_dto = LedgerDto { accounts };

        let serialized = serde_json::to_value(&ledger_dto).unwrap();

        let expected = json!({
            "accounts": {
                "nano_1ipx847tk8o46pwxt5qjdbncjqcbwcc1rrmqnkztrfjy5k7z4imsrata9est": {
                    "frontier": "000D1BAEC8EC208142C99059B393051BAC8380F9B5A2E6B2489A277D81789F3F",
                    "open_block": "991CF190094C00F0B68E2E5F75F6BEE95A2E0BD93CEAA4A6734DB9F19C34F1ED",
                    "representative_block": "991CF190094C00F0B68E2E5F75F6BEE95A2E0BD93CEAA4A6734DB9F19C34F1ED",
                    "balance": "10000000000000000000000000000000",
                    "modified_timestamp": 1553174994,
                    "block_count": 50,
                    "representative": "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
                    "weight": "10000000000000000000000000000000",
                    "pending": "10000000000000000000000000000",
                    "receivable": "10000000000000000000000000000"
                }
            }
        });

        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_ledger_dto_deserialization() {
        let json_str = r#"{
            "accounts": {
                "nano_1ipx847tk8o46pwxt5qjdbncjqcbwcc1rrmqnkztrfjy5k7z4imsrata9est": {
                    "frontier": "000D1BAEC8EC208142C99059B393051BAC8380F9B5A2E6B2489A277D81789F3F",
                    "open_block": "991CF190094C00F0B68E2E5F75F6BEE95A2E0BD93CEAA4A6734DB9F19C34F1ED",
                    "representative_block": "991CF190094C00F0B68E2E5F75F6BEE95A2E0BD93CEAA4A6734DB9F19C34F1ED",
                    "balance": "10000000000000000000000000000000",
                    "modified_timestamp": 1553174994,
                    "block_count": 50,
                    "representative": "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
                    "weight": "10000000000000000000000000000000",
                    "pending": "10000000000000000000000000000",
                    "receivable": "10000000000000000000000000000"
                }
            }
        }"#;

        let deserialized: LedgerDto = serde_json::from_str(json_str).unwrap();

        assert_eq!(deserialized.accounts.len(), 1);

        let account = Account::decode_account(
            "nano_1ipx847tk8o46pwxt5qjdbncjqcbwcc1rrmqnkztrfjy5k7z4imsrata9est",
        )
        .unwrap();
        let account_info = deserialized.accounts.get(&account).unwrap();

        assert_eq!(
            account_info.frontier,
            BlockHash::decode_hex(
                "000D1BAEC8EC208142C99059B393051BAC8380F9B5A2E6B2489A277D81789F3F"
            )
            .unwrap()
        );
        assert_eq!(
            account_info.open_block,
            BlockHash::decode_hex(
                "991CF190094C00F0B68E2E5F75F6BEE95A2E0BD93CEAA4A6734DB9F19C34F1ED"
            )
            .unwrap()
        );
        assert_eq!(
            account_info.representative_block,
            BlockHash::decode_hex(
                "991CF190094C00F0B68E2E5F75F6BEE95A2E0BD93CEAA4A6734DB9F19C34F1ED"
            )
            .unwrap()
        );
        assert_eq!(
            account_info.balance,
            Amount::raw(10000000000000000000000000000000u128)
        );
        assert_eq!(account_info.modified_timestamp, 1553174994);
        assert_eq!(account_info.block_count, 50);
        assert_eq!(
            account_info.representative,
            Some(
                Account::decode_account(
                    "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3"
                )
                .unwrap()
            )
        );
        assert_eq!(
            account_info.weight,
            Some(Amount::raw(10000000000000000000000000000000u128))
        );
        assert_eq!(
            account_info.pending,
            Some(Amount::raw(10000000000000000000000000000u128))
        );
        assert_eq!(
            account_info.receivable,
            Some(Amount::raw(10000000000000000000000000000u128))
        );
    }
}
