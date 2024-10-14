use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UptimeDto {
    pub seconds: u64,
}

impl UptimeDto {
    pub fn new(seconds: u64) -> Self {
        Self { seconds }
    }
}