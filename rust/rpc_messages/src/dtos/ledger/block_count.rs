use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlockCountDto {
    pub count: u64,
    pub unchecked: u64,
    pub cemented: u64,
}

impl BlockCountDto {
    pub fn new(count: u64, unchecked: u64, cemented: u64) -> Self {
        Self {
            count,
            unchecked,
            cemented,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BlockCountDto;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_block_count_dto() {
        let block_count_dto = BlockCountDto::new(1, 1, 1);
        assert_eq!(
            serde_json::to_string_pretty(&block_count_dto).unwrap(),
            r#"{
  "count": 1,
  "unchecked": 1,
  "cemented": 1
}"#
        );
    }

    #[test]
    fn deserialize_block_account_dto() {
        let bool_dto = BlockCountDto::new(1, 1, 1);
        let serialized = to_string_pretty(&bool_dto).unwrap();
        let deserialized: BlockCountDto = from_str(&serialized).unwrap();
        assert_eq!(bool_dto, deserialized);
    }
}
