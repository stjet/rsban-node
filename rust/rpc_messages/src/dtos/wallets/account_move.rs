use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountMoveDto {
    pub moved: bool,
}

impl AccountMoveDto {
    pub fn new(moved: bool) -> Self {
        Self { moved }
    }
}
