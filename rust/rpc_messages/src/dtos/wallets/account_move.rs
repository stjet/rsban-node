use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountMovedDto {
    pub moved: bool,
}

impl AccountMovedDto {
    pub fn new(moved: bool) -> Self {
        Self { moved }
    }
}
