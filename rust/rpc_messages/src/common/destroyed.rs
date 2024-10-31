use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DestroyedResponse {
    pub destroyed: String,
}

impl DestroyedResponse {
    pub fn new(destroyed: bool) -> Self {
        Self {
            destroyed: if destroyed {
                "1".to_string()
            } else {
                "0".to_string()
            },
        }
    }
}
