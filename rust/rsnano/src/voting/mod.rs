mod local_vote_history;
mod vote;
use std::sync::RwLock;

pub(crate) use local_vote_history::*;
pub(crate) use vote::*;

use crate::Uniquer;

pub(crate) type VoteUniquer = Uniquer<RwLock<Vote>>;
