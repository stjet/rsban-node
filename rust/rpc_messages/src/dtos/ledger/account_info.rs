use rsnano_core::Account;
use rsnano_core::{Amount, BlockHash};
use serde::{Deserialize, Serialize};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub representative: Option<Account>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<Amount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<Amount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receivable: Option<Amount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_balance: Option<Amount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_pending: Option<Amount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_receivable: Option<Amount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_representative: Option<Account>,
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
            representative: None,
            weight: None,
            pending: None,
            receivable: None,
            confirmed_balance: None,
            confirmed_pending: None,
            confirmed_receivable: None,
            confirmed_representative: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_account_info_dto_with_none_values() {
        let account_info = AccountInfoDto {
            frontier: BlockHash::zero(),
            open_block: BlockHash::zero(),
            representative_block: BlockHash::zero(),
            balance: Amount::raw(1000),
            modified_timestamp: 1234567890,
            block_count: 100,
            account_version: 1,
            confirmed_height: Some(99),
            confirmation_height_frontier: Some(BlockHash::zero()),
            representative: Some(Account::zero()),
            weight: Some(Amount::raw(2000)),
            pending: Some(Amount::raw(300)),
            receivable: Some(Amount::raw(400)),
            confirmed_balance: None,
            confirmed_pending: None,
            confirmed_receivable: None,
            confirmed_representative: None,
        };

        let serialized = to_string_pretty(&account_info).unwrap();
        let deserialized: AccountInfoDto = from_str(&serialized).unwrap();

        assert_eq!(account_info, deserialized);
    }

    #[test]
    fn deserialize_account_info_dto_with_none_values() {
        let account_info = AccountInfoDto::new(
            BlockHash::zero(),
            BlockHash::zero(),
            BlockHash::zero(),
            Amount::raw(1000),
            1234567890,
            100,
            1,
        );

        let serialized = to_string_pretty(&account_info).unwrap();
        let deserialized: AccountInfoDto = from_str(&serialized).unwrap();

        assert_eq!(account_info, deserialized);
        assert!(!serialized.contains("weight"));
        assert!(!serialized.contains("pending"));
        assert!(!serialized.contains("receivable"));
        assert!(!serialized.contains("confirmed_balance"));
        assert!(!serialized.contains("confirmed_pending"));
        assert!(!serialized.contains("confirmed_receivable"));
        assert!(!serialized.contains("confirmed_representative"));
    }

    fn create_account_info_dto_with_some_values() -> AccountInfoDto {
        AccountInfoDto {
            frontier: BlockHash::zero(),
            open_block: BlockHash::zero(),
            representative_block: BlockHash::zero(),
            balance: Amount::from(1000),
            modified_timestamp: 1234567890,
            block_count: 100,
            account_version: 1,
            confirmed_height: Some(99),
            confirmation_height_frontier: Some(BlockHash::zero()),
            representative: Some(Account::zero()),
            weight: Some(Amount::from(2000)),
            pending: Some(Amount::from(300)),
            receivable: Some(Amount::from(400)),
            confirmed_balance: Some(Amount::from(950)),
            confirmed_pending: Some(Amount::from(250)),
            confirmed_receivable: Some(Amount::from(350)),
            confirmed_representative: Some(Account::zero()),
        }
    }

    #[test]
    fn serialize_account_info_dto_with_some_values() {
        let account_info = create_account_info_dto_with_some_values();
        let serialized = to_string_pretty(&account_info).unwrap();

        assert!(serialized.contains("frontier"));
        assert!(serialized.contains("representative"));
        assert!(serialized.contains("weight"));
        assert!(serialized.contains("pending"));
        assert!(serialized.contains("receivable"));
        assert!(serialized.contains("confirmed_balance"));
        assert!(serialized.contains("confirmed_pending"));
        assert!(serialized.contains("confirmed_receivable"));
        assert!(serialized.contains("confirmed_representative"));
    }

    #[test]
    fn deserialize_account_info_dto_with_some_values() {
        let account_info = create_account_info_dto_with_some_values();
        let serialized = to_string_pretty(&account_info).unwrap();
        let deserialized: AccountInfoDto = from_str(&serialized).unwrap();

        assert_eq!(account_info, deserialized);
    }
}
