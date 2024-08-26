use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountRemoveDto {
    pub removed: bool,
}

impl AccountRemoveDto {
    pub fn new(removed: bool) -> Self {
        Self { removed }
    }
}
