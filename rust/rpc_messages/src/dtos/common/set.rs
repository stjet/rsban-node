use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SetDto {
    pub set: bool,
}

impl SetDto {
    pub fn new(set: bool) -> Self {
        Self { set }
    }
}
