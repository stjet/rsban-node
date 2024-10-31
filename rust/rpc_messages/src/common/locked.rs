use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LockedResponse {
    pub locked: String,
}

impl LockedResponse {
    pub fn new(locked: bool) -> Self {
        Self {
            locked: if locked {
                "1".to_owned()
            } else {
                "0".to_owned()
            },
        }
    }
}
