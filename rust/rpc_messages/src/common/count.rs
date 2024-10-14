use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CountDto {
    pub count: u64,
}

impl CountDto {
    pub fn new(count: u64) -> Self {
        Self { count }
    }
}