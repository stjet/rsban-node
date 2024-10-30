use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CountArgs {
    pub count: Option<u64>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CountResponse {
    pub count: u64,
}

impl CountResponse {
    pub fn new(count: u64) -> Self {
        Self { count }
    }
}
