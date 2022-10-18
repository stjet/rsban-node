mod local_vote_history;
mod vote;
mod vote_spacing;

pub(crate) use local_vote_history::*;
pub(crate) use vote::*;
pub use vote_spacing::VoteSpacing;

use std::sync::RwLock;
use crate::core::Uniquer;

pub(crate) type VoteUniquer = Uniquer<RwLock<Vote>>;
