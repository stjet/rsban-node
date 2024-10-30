use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ValidResponse {
    pub valid: String,
}

impl ValidResponse {
    pub fn new(valid: bool) -> Self {
        Self {
            valid: if valid {
                "1".to_owned()
            } else {
                "0".to_owned()
            },
        }
    }
}
