use rsnano_core::{BlockHash, WorkNonce};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkValidateDto {
    pub valid_all: bool,
    pub valid_receive: bool,
    pub difficulty: u64,
    pub multiplier: f64,
}

impl WorkValidateDto {
    pub fn new(valid_all: bool, valid_receive: bool, difficulty: u64, multiplier: f64) -> Self {
        Self {
            valid_all,
            valid_receive,
            difficulty,
            multiplier,
        }
    }
}
