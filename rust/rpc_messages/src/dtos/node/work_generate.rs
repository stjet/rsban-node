use rsnano_core::{BlockHash, WorkNonce};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkGenerateDto {
    pub work: WorkNonce,
    pub difficulty: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiplier: Option<f64>,
    pub hash: BlockHash,
}

impl WorkGenerateDto {
    pub fn new(work: WorkNonce, difficulty: u64, multiplier: Option<f64>, hash: BlockHash) -> Self {
        Self {
            work,
            difficulty,
            multiplier,
            hash,
        }
    }
}
