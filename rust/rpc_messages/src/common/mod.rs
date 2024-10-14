mod account_balance;
mod accounts;
mod accounts_balances;
mod blocks;
mod error;
mod frontiers;
mod key_pair;
mod public_key;
mod receivable;
mod success;
mod account;
mod exists;
mod hash;
mod removed;
mod moved;
mod destroyed;
mod locked;
mod accounts_with_amounts;
mod set;
mod representative;
//mod block_count;
mod weight;
mod seconds;
mod valid;
//mod count;
mod amount;
mod started;
mod block;
mod available;

pub use account_balance::*;
pub use accounts::*;
pub use accounts_balances::*;
pub use blocks::*;
pub use error::*;
pub use frontiers::*;
pub use key_pair::*;
pub use public_key::*;
pub use receivable::*;
pub use success::*;
pub use account::*;
pub use exists::*;
pub use hash::*;
pub use removed::*;
pub use moved::*;
pub use destroyed::*;
pub use locked::*;
//pub use block_count::*;
pub use weight::*;
pub use seconds::*;
//pub use count::*;
pub use amount::*;
pub use started::*;
pub use valid::*;
pub use block::*;
pub use accounts_with_amounts::*;
pub use set::*;
pub use representative::*;
pub use available::*;

use rsnano_core::{BlockType, WorkVersion};
use serde::{Deserialize, Serialize};

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

#[derive(PartialEq, Eq, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockTypeDto {
    Send,
    Receive,
    Open,
    Change,
    State,
}

impl From<BlockType> for BlockTypeDto {
    fn from(value: BlockType) -> Self {
        match value {
            BlockType::LegacySend => BlockTypeDto::Send,
            BlockType::LegacyReceive => BlockTypeDto::Receive,
            BlockType::LegacyOpen => BlockTypeDto::Open,
            BlockType::LegacyChange => BlockTypeDto::Change,
            BlockType::State => BlockTypeDto::State,
            BlockType::Invalid | BlockType::NotABlock => unimplemented!(),
        }
    }
}

impl From<BlockTypeDto> for BlockType {
    fn from(value: BlockTypeDto) -> Self {
        match value {
            BlockTypeDto::Send => BlockType::LegacySend,
            BlockTypeDto::Receive => BlockType::LegacyReceive,
            BlockTypeDto::Open => BlockType::LegacyOpen,
            BlockTypeDto::Change => BlockType::LegacyChange,
            BlockTypeDto::State => BlockType::State,
        }
    }
}
