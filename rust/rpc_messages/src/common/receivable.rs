use indexmap::IndexMap;
use rsnano_core::{Account, Amount, BlockHash};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ReceivableResponse {
    Simple(ReceivableSimple),
    Source(ReceivableSource),
    Threshold(ReceivableThreshold),
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ReceivableSimple {
    pub blocks: Vec<BlockHash>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ReceivableThreshold {
    pub blocks: IndexMap<BlockHash, Amount>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ReceivableSource {
    pub blocks: IndexMap<BlockHash, SourceInfo>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AccountsReceivableResponse {
    Simple(AccountsReceivableSimple),
    Source(AccountsReceivableSource),
    Threshold(AccountsReceivableThreshold),
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsReceivableSimple {
    pub blocks: IndexMap<Account, Vec<BlockHash>>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsReceivableThreshold {
    pub blocks: IndexMap<Account, IndexMap<BlockHash, Amount>>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountsReceivableSource {
    pub blocks: IndexMap<Account, IndexMap<BlockHash, SourceInfo>>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SourceInfo {
    pub amount: Amount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Account>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_version: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_wallet_receivable_dto_blocks() {
        let mut blocks = IndexMap::new();
        blocks.insert(Account::zero(), vec![BlockHash::zero()]);
        let works = AccountsReceivableResponse::Simple(AccountsReceivableSimple { blocks });
        let expected_json = r#"{"blocks":{"nano_1111111111111111111111111111111111111111111111111111hifc8npp":["0000000000000000000000000000000000000000000000000000000000000000"]}}"#;

        let serialized = serde_json::to_string(&works).unwrap();

        assert_eq!(serialized, expected_json);
    }

    #[test]
    fn deserialize_wallet_receivable_dto_blocks() {
        let json_data = r#"{"blocks":{"nano_1111111111111111111111111111111111111111111111111111hifc8npp":["0000000000000000000000000000000000000000000000000000000000000000"]}}"#;
        let works: AccountsReceivableResponse = serde_json::from_str(json_data).unwrap();

        let mut expected_blocks = IndexMap::new();
        expected_blocks.insert(Account::zero(), vec![BlockHash::zero()]);

        let expected_works = AccountsReceivableResponse::Simple(AccountsReceivableSimple {
            blocks: expected_blocks,
        });

        assert_eq!(works, expected_works);
    }

    #[test]
    fn serialize_wallet_receivable_dto_threshold() {
        let mut blocks = IndexMap::new();
        let mut inner_map = IndexMap::new();
        inner_map.insert(BlockHash::zero(), Amount::from(1000));
        blocks.insert(Account::zero(), inner_map);

        let works = AccountsReceivableResponse::Threshold(AccountsReceivableThreshold { blocks });

        let expected_json = r#"{"blocks":{"nano_1111111111111111111111111111111111111111111111111111hifc8npp":{"0000000000000000000000000000000000000000000000000000000000000000":"1000"}}}"#;
        let serialized = serde_json::to_string(&works).unwrap();

        assert_eq!(serialized, expected_json);
    }

    #[test]
    fn deserialize_wallet_receivable_dto_threshold() {
        let json_data = r#"{"blocks":{"nano_1111111111111111111111111111111111111111111111111111hifc8npp":{"0000000000000000000000000000000000000000000000000000000000000000":"1000"}}}"#;
        let works: AccountsReceivableResponse = serde_json::from_str(json_data).unwrap();

        let mut expected_blocks = IndexMap::new();
        let mut inner_map = IndexMap::new();
        inner_map.insert(BlockHash::zero(), Amount::from(1000));
        expected_blocks.insert(Account::zero(), inner_map);

        let expected_works = AccountsReceivableResponse::Threshold(AccountsReceivableThreshold {
            blocks: expected_blocks,
        });

        assert_eq!(works, expected_works);
    }

    #[test]
    fn serialize_wallet_receivable_dto_source() {
        let mut blocks = IndexMap::new();
        let mut inner_map = IndexMap::new();
        inner_map.insert(
            BlockHash::zero(),
            SourceInfo {
                amount: Amount::from(1000),
                source: Some(Account::zero()),
                min_version: None,
            },
        );
        blocks.insert(Account::zero(), inner_map);

        let works = AccountsReceivableResponse::Source(AccountsReceivableSource { blocks });

        let expected_json = r#"{"blocks":{"nano_1111111111111111111111111111111111111111111111111111hifc8npp":{"0000000000000000000000000000000000000000000000000000000000000000":{"amount":"1000","source":"nano_1111111111111111111111111111111111111111111111111111hifc8npp"}}}}"#;
        let serialized = serde_json::to_string(&works).unwrap();

        assert_eq!(serialized, expected_json);
    }

    #[test]
    fn deserialize_wallet_receivable_dto_source() {
        let json_data = r#"{"blocks":{"nano_1111111111111111111111111111111111111111111111111111hifc8npp":{"0000000000000000000000000000000000000000000000000000000000000000":{"amount":"1000","source":"nano_1111111111111111111111111111111111111111111111111111hifc8npp"}}}}"#;
        let works: AccountsReceivableResponse = serde_json::from_str(json_data).unwrap();

        let mut expected_blocks = IndexMap::new();
        let mut inner_map = IndexMap::new();
        inner_map.insert(
            BlockHash::zero(),
            SourceInfo {
                amount: Amount::from(1000),
                source: Some(Account::zero()),
                min_version: None,
            },
        );
        expected_blocks.insert(Account::zero(), inner_map);

        let expected_works = AccountsReceivableResponse::Source(AccountsReceivableSource {
            blocks: expected_blocks,
        });

        assert_eq!(works, expected_works);
    }
}
