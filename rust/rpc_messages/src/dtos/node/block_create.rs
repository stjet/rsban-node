use rsnano_core::{BlockHash, JsonBlock, WorkNonce};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlockCreateDto {
    pub hash: BlockHash,
    pub difficulty: WorkNonce,
    pub block: JsonBlock,
}

impl BlockCreateDto {
    pub fn new(hash: BlockHash, difficulty: WorkNonce, block: JsonBlock) -> Self {
        Self {
            hash,
            difficulty,
            block,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{Block, StateBlock};
    use serde_json::json;

    #[test]
    fn serialize_block_create_dto() {
        let block = StateBlock::new_test_instance();

        let dto = BlockCreateDto::new(block.hash(), 10.into(), block.json_representation());

        let serialized = serde_json::to_string_pretty(&dto).unwrap();
        let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        let expected_json = json!({
            "hash": block.hash(),
            "difficulty": "000000000000000A",
            "block": block.json_representation()
        });

        assert_eq!(deserialized, expected_json);
    }

    #[test]
    fn deserialize_block_create_dto() {
        let block = StateBlock::new_test_instance();

        let json = json!({
            "hash": block.hash(),
            "difficulty": "000000000000000A",
            "block": block.json_representation()
        });

        let json_string = serde_json::to_string(&json).unwrap();

        let dto: BlockCreateDto = serde_json::from_str(&json_string).unwrap();

        assert_eq!(dto.hash, block.hash());
        assert_eq!(dto.difficulty, 10.into());
        assert_eq!(dto.block, block.json_representation());
    }
}
