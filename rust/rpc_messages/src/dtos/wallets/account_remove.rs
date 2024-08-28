use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountRemovedDto {
    pub removed: bool,
}

impl AccountRemovedDto {
    pub fn new(removed: bool) -> Self {
        Self { removed }
    }
}
