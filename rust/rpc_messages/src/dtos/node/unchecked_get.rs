use rsnano_core::JsonBlock;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UncheckedGetDto {
    pub modified_timestamp: u64,
    pub contents: JsonBlock,
}

impl UncheckedGetDto {
    pub fn new(modified_timestamp: u64, contents: JsonBlock) -> Self {
        Self {
            modified_timestamp,
            contents,
        }
    }
}
