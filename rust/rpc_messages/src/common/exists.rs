use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ExistsDto {
    pub exists: String,
}

impl ExistsDto {
    pub fn new(exists: bool) -> Self {
        Self {
            exists: if exists {
                "1".to_owned()
            } else {
                "0".to_owned()
            },
        }
    }
}
