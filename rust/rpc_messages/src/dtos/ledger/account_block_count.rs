use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountBlockCountDto {
    pub block_count: u64,
}

impl AccountBlockCountDto {
    pub fn new(block_count: u64) -> Self {
        Self { block_count }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, to_string};

    #[test]
    fn serialize_block_count_dto() {
        let dto = AccountBlockCountDto::new(1);

        let serialized = to_string(&dto).unwrap();

        let expected_json = serde_json::json!({
            "block_count": 1
        });

        let actual_json: serde_json::Value = from_str(&serialized).unwrap();
        assert_eq!(actual_json, expected_json);
    }

    #[test]
    fn deserialize_account_block_count_dto() {
        let json_str = r#"{
            "block_count": 1
        }"#;

        let deserialized: AccountBlockCountDto = from_str(json_str).unwrap();

        let expected = AccountBlockCountDto::new(1);

        assert_eq!(deserialized, expected);
    }
}
