

use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LockDto {
    pub lock: bool,
}

impl LockDto {
    pub fn new(lock: bool) -> Self {
        Self { lock }
    }
}
