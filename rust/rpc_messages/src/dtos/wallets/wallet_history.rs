use rsnano_core::{Account, Amount, BlockHash, BlockSubType};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, Clone)]
pub struct WalletHistoryDto {
    pub history: Vec<HistoryEntryDto>,
}

impl WalletHistoryDto {
    pub fn new(history: Vec<HistoryEntryDto>) -> Self {
        Self { history }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, Clone)]
pub struct HistoryEntryDto {
    #[serde(rename = "type")]
    pub entry_type: BlockSubType,
    pub account: Account,
    pub amount: Amount,
    pub block_account: Account,
    pub hash: BlockHash,
    pub local_timestamp: u64,
}

impl HistoryEntryDto {
    pub fn new(
        entry_type: BlockSubType,
        account: Account,
        amount: Amount,
        block_account: Account,
        hash: BlockHash,
        local_timestamp: u64,
    ) -> Self {
        Self {
            entry_type,
            account,
            amount,
            block_account,
            hash,
            local_timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn serialize_wallet_history_dto() {
        let history_entry = HistoryEntryDto {
            entry_type: BlockSubType::Send,
            account: Account::decode_account(
                "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
            )
            .unwrap(),
            amount: Amount::raw(1000000000000000000000000000000),
            block_account: Account::decode_account(
                "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
            )
            .unwrap(),
            hash: BlockHash::decode_hex(
                "000D1BAEC8EC208142C99059B393051BAC8380F9B5A2E6B2489A277D81789F3F",
            )
            .unwrap(),
            local_timestamp: 1625097600,
        };

        let wallet_history_dto = WalletHistoryDto {
            history: vec![history_entry],
        };

        let expected_json = r#"{
  "history": [
    {
      "type": "send",
      "account": "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
      "amount": "1000000000000000000000000000000",
      "block_account": "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
      "hash": "000D1BAEC8EC208142C99059B393051BAC8380F9B5A2E6B2489A277D81789F3F",
      "local_timestamp": 1625097600
    }
  ]
}"#;

        let serialized = serde_json::to_string_pretty(&wallet_history_dto).unwrap();
        assert_eq!(serialized, expected_json);
    }

    #[test]
    fn deserialize_wallet_history_dto() {
        let json_data = r#"{
  "history": [
    {
      "type": "send",
      "account": "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
      "amount": "1000000000000000000000000000000",
      "block_account": "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
      "hash": "000D1BAEC8EC208142C99059B393051BAC8380F9B5A2E6B2489A277D81789F3F",
      "local_timestamp": 1625097600
    }
  ]
}"#;

        let expected_history_entry = HistoryEntryDto {
            entry_type: BlockSubType::Send,
            account: Account::decode_account(
                "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
            )
            .unwrap(),
            amount: Amount::raw(1000000000000000000000000000000),
            block_account: Account::decode_account(
                "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3",
            )
            .unwrap(),
            hash: BlockHash::decode_hex(
                "000D1BAEC8EC208142C99059B393051BAC8380F9B5A2E6B2489A277D81789F3F",
            )
            .unwrap(),
            local_timestamp: 1625097600,
        };

        let expected_wallet_history_dto = WalletHistoryDto {
            history: vec![expected_history_entry],
        };

        let deserialized: WalletHistoryDto = serde_json::from_str(json_data).unwrap();
        assert_eq!(deserialized, expected_wallet_history_dto);
    }
}
