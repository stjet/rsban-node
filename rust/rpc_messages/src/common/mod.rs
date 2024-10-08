mod account_balance;
mod account_with_count;
mod accounts;
mod accounts_balances;
mod blocks;
mod dynamic_key;
mod error;
mod frontiers;
mod key_pair;
mod ledger;
mod public_key;
mod receivable;
mod success;

pub use account_balance::*;
pub use account_with_count::*;
pub use accounts::*;
pub use accounts_balances::*;
pub use blocks::*;
pub use dynamic_key::*;
pub use error::*;
pub use frontiers::*;
pub use key_pair::*;
pub use ledger::*;
pub use public_key::*;
pub use receivable::*;
use rsnano_core::WorkVersion;
use serde::{Deserialize, Serialize};
pub use success::*;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkVersionDto {
    Work1,
}

impl From<WorkVersion> for WorkVersionDto {
    fn from(value: WorkVersion) -> Self {
        match value {
            WorkVersion::Unspecified => unimplemented!(),
            WorkVersion::Work1 => WorkVersionDto::Work1,
        }
    }
}

impl From<WorkVersionDto> for WorkVersion {
    fn from(value: WorkVersionDto) -> Self {
        match value {
            WorkVersionDto::Work1 => WorkVersion::Work1,
        }
    }
}
