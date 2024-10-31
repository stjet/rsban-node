use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct StartedResponse {
    pub started: String,
}

impl StartedResponse {
    pub fn new(started: bool) -> Self {
        Self {
            started: if started {
                "1".to_string()
            } else {
                "0".to_string()
            },
        }
    }
}
