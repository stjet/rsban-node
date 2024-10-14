use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BootstrapLazyDto {
    pub started: bool,
    pub key_inserted: bool,
}

impl BootstrapLazyDto {
    pub fn new(started: bool, key_inserted: bool) -> Self {
        Self {
            started,
            key_inserted,
        }
    }
}
